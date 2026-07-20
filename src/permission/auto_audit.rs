use crate::config::AppConfig;
use crate::llm::{ChatMessage, ChatStreamEvent, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use crate::permission::{decide_permission, PermissionDecision};
use crate::prompts;
use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// LLM 自动审核的结构化结果。
#[derive(Debug, Clone, Deserialize)]
struct AutoAuditLlmResponse {
    decision: String,
    #[serde(default)]
    reason: Option<String>,
}

/// 为自动审核构造运行时客户端；未配置专用模型时继承会话模型。
///
/// 参数:
/// - `config`: 已包含本轮会话模型覆盖的配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - 自动审核客户端
pub(crate) fn resolve_auto_audit_client(
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<OpenAiCompatibleClient> {
    let runtime = auto_audit_runtime_config(config)?;
    OpenAiCompatibleClient::from_config(&runtime, paths)
}

/// 构造自动审核使用的配置副本。
///
/// 参数:
/// - `config`: 当前会话配置
///
/// 返回:
/// - 已应用自动审核模型选择的配置
fn auto_audit_runtime_config(config: &AppConfig) -> Result<AppConfig> {
    let provider_id = config.permission.auto_audit_provider_id.trim();
    let model = config.permission.auto_audit_model.trim();
    match (provider_id.is_empty(), model.is_empty()) {
        (true, true) => Ok(config.clone()),
        (false, false) => {
            let mut runtime = config.clone();
            runtime.set_active_provider_model(provider_id, model)?;
            Ok(runtime)
        }
        _ => bail!(
            "permission.auto_audit_provider_id and permission.auto_audit_model must be provided together"
        ),
    }
}

/// 调用 LLM 审核工具，并在成功时提交权限决定。
///
/// 若人工已先决定，`decide_permission` 会失败，此时静默退出。
///
/// 参数:
/// - `client`: 自动审核模型客户端
/// - `request_id`: 权限请求标识
/// - `tool`: 工具名
/// - `arguments`: 工具参数文本
/// - `context`: 少量上下文摘要
///
/// 返回:
/// - 审核是否成功提交决定（人工先到时返回 false）
pub(crate) async fn run_auto_audit(
    client: &OpenAiCompatibleClient,
    request_id: &str,
    tool: &str,
    arguments: &str,
    context: &str,
) -> Result<bool> {
    // 1. 组装审核消息
    let user = format!(
        "Tool: {tool}\nArguments:\n{arguments}\n\nRecent context:\n{context}\n"
    );
    let messages = vec![
        ChatMessage::system(prompts::AUTO_AUDIT_SYSTEM_PROMPT),
        ChatMessage::plain("user", user),
    ];
    // 2. 流式收集正文
    let result = client
        .chat_stream_events(messages, Vec::new(), |_event: ChatStreamEvent| Ok(()))
        .await
        .context("auto-audit model request failed")?;
    let content = result.content.trim();
    let decision = parse_auto_audit_response(content)?;
    // 3. 提交决定；若人工已先处理则请求已不存在
    match decide_permission(request_id, decision) {
        Ok(()) => Ok(true),
        Err(error) => {
            let message = error.to_string();
            if message.contains("no longer pending") || message.contains("no longer running") {
                Ok(false)
            } else {
                Err(error)
            }
        }
    }
}

/// 解析 LLM JSON 审核结果。
///
/// 参数:
/// - `content`: 模型输出
///
/// 返回:
/// - 权限决定
fn parse_auto_audit_response(content: &str) -> Result<PermissionDecision> {
    let json_text = extract_json_object(content).unwrap_or(content);
    let parsed: AutoAuditLlmResponse = serde_json::from_str(json_text)
        .with_context(|| format!("invalid auto-audit JSON: {content}"))?;
    let decision = parsed.decision.trim().to_ascii_lowercase();
    let reason = parsed
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    match decision.as_str() {
        "allow" | "approve" | "approved" => Ok(PermissionDecision::Allow),
        "deny" | "reject" | "rejected" | "block" | "blocked" => Ok(PermissionDecision::Deny {
            reply: Some(reason.unwrap_or_else(|| "自动审核拒绝".to_string())),
        }),
        other => bail!("unknown auto-audit decision: {other}"),
    }
}

/// 从可能含前后说明的文本中截取首个 JSON 对象。
fn extract_json_object(content: &str) -> Option<&str> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    if end >= start {
        Some(&content[start..=end])
    } else {
        None
    }
}

/// 从近期消息构造自动审核上下文摘要。
///
/// 参数:
/// - `messages`: 会话消息列表
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 压缩后的上下文文本
pub(crate) fn build_audit_context(messages: &[ChatMessage], max_chars: usize) -> String {
    let mut parts = Vec::new();
    for message in messages.iter().rev().take(8) {
        let role = message.role.as_str();
        let text = message_text(message);
        if text.trim().is_empty() {
            continue;
        }
        let snippet = if text.chars().count() > 400 {
            let clipped: String = text.chars().take(400).collect();
            format!("{clipped}…")
        } else {
            text
        };
        parts.push(format!("[{role}] {snippet}"));
        let joined = parts.join("\n");
        if joined.chars().count() >= max_chars {
            break;
        }
    }
    parts.reverse();
    let mut out = parts.join("\n");
    if out.chars().count() > max_chars {
        out = out.chars().skip(out.chars().count() - max_chars).collect();
    }
    if out.is_empty() {
        "(no recent messages)".to_string()
    } else {
        out
    }
}

/// 提取消息纯文本内容。
fn message_text(message: &ChatMessage) -> String {
    match &message.content {
        Some(crate::llm::ChatContent::Text(text)) => text.clone(),
        Some(crate::llm::ChatContent::Parts(parts)) => parts
            .iter()
            .filter_map(|part| match part {
                crate::llm::ChatContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_allow_and_deny_json() {
        let allow = parse_auto_audit_response(r#"{"decision":"allow","reason":"safe"}"#).unwrap();
        assert!(matches!(allow, PermissionDecision::Allow));
        let deny = parse_auto_audit_response(
            r#"prefix {"decision":"deny","reason":"risk"} suffix"#,
        )
        .unwrap();
        match deny {
            PermissionDecision::Deny { reply } => assert_eq!(reply.as_deref(), Some("risk")),
            _ => panic!("expected deny"),
        }
    }
}

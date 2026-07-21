use crate::config::AppConfig;
use crate::llm::{ChatMessage, ChatStreamEvent, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use crate::state::{rename_session, SessionInfo};
use anyhow::{bail, Result};
use std::time::Duration;
use tokio::time::timeout;

const TITLE_TIMEOUT: Duration = Duration::from_secs(20);

/// 为标题总结构造运行时客户端；未配置专用模型时继承当前配置模型。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - 标题总结客户端
pub(crate) fn resolve_title_client(config: &AppConfig, paths: &SaiPaths) -> Result<OpenAiCompatibleClient> {
    let runtime = title_runtime_config(config)?;
    OpenAiCompatibleClient::from_config(&runtime, paths)
}

/// 构造标题总结使用的配置副本。
fn title_runtime_config(config: &AppConfig) -> Result<AppConfig> {
    let provider_id = config.session.auto_title_provider_id.trim();
    let model = config.session.auto_title_model.trim();
    match (provider_id.is_empty(), model.is_empty()) {
        (true, true) => Ok(config.clone()),
        (false, false) => {
            let mut runtime = config.clone();
            runtime.set_active_provider_model(provider_id, model)?;
            Ok(runtime)
        }
        _ => bail!(
            "session.auto_title_provider_id and session.auto_title_model must be provided together"
        ),
    }
}

/// 使用小模型为会话生成简洁标题。
///
/// 参数:
/// - `client`: 模型客户端
/// - `user_message`: 首轮用户消息
/// - `assistant_preview`: 可选助手回复摘要
///
/// 返回:
/// - 清洗后的标题（过长会截断）
pub(crate) async fn summarize_session_title(
    client: &OpenAiCompatibleClient,
    user_message: &str,
    assistant_preview: Option<&str>,
) -> Result<String> {
    let user = {
        let mut body = format!("User message:\n{}\n", truncate(user_message, 1200));
        if let Some(preview) = assistant_preview.map(str::trim).filter(|s| !s.is_empty()) {
            body.push_str(&format!(
                "\nAssistant reply preview:\n{}\n",
                truncate(preview, 800)
            ));
        }
        body
    };
    let messages = vec![
        ChatMessage::system(
            "You name chat sessions. Reply with ONLY a short title (max 24 Chinese characters or 8 English words). No quotes, no punctuation wrappers, no explanation.",
        ),
        ChatMessage::plain("user", user),
    ];
    let result = match timeout(
        TITLE_TIMEOUT,
        client.chat_stream_events(messages, Vec::new(), |_event: ChatStreamEvent| Ok(())),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => return Err(error),
        Err(_) => bail!("session title generation timed out"),
    };
    Ok(sanitize_title(&result.content))
}

/// 若会话仍为默认标题，则生成并写回标题。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `session_id`: 会话 ID
/// - `current_title`: 当前标题
/// - `user_message`: 首轮用户输入
/// - `assistant_preview`: 可选助手预览
///
/// 返回:
/// - 更新后的会话；未改动时返回 None
pub(crate) async fn maybe_auto_title_session(
    paths: &SaiPaths,
    config: &AppConfig,
    session_id: &str,
    current_title: &str,
    user_message: &str,
    assistant_preview: Option<&str>,
) -> Result<Option<SessionInfo>> {
    if !config.session.auto_title_enabled {
        return Ok(None);
    }
    // 仅首次：占位标题，或仍是首条用户消息的截断启发式标题
    let heuristic = crate::state::title_from_message_public(user_message, current_title);
    if !is_placeholder_title(current_title) && current_title != heuristic {
        return Ok(None);
    }
    if user_message.trim().is_empty() {
        return Ok(None);
    }
    let client = resolve_title_client(config, paths)?;
    let title = match summarize_session_title(&client, user_message, assistant_preview).await {
        Ok(title) if !title.is_empty() => title,
        Ok(_) => return Ok(None),
        Err(_) => {
            let fallback =
                crate::state::title_from_message_public(user_message, current_title);
            if fallback == current_title {
                return Ok(None);
            }
            fallback
        }
    };
    if title == current_title {
        return Ok(None);
    }
    let session = rename_session(paths, session_id, &title)?;
    Ok(Some(session))
}

/// 是否为占位标题（允许首次自动命名）。
pub(crate) fn is_placeholder_title(title: &str) -> bool {
    matches!(
        title.trim(),
        "" | "New session" | "Default" | "新会话" | "默认会话"
    )
}

/// 清洗模型输出为可用标题。
fn sanitize_title(raw: &str) -> String {
    let line = raw
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '“' | '”' | '‘' | '’' | '《' | '》' | '「' | '」' | '【' | '】'
            )
        })
        .trim_start_matches(|ch: char| matches!(ch, '#' | '-' | '*' | '·' | '•'))
        .trim();
    let cleaned = line.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&cleaned, 48).trim().to_string()
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_quoted_titles() {
        assert_eq!(sanitize_title("\"修复登录错误\"\nextra"), "修复登录错误");
        assert_eq!(sanitize_title("## Fix auth bug"), "Fix auth bug");
    }

    #[test]
    fn detects_placeholder_titles() {
        assert!(is_placeholder_title("New session"));
        assert!(is_placeholder_title("Default"));
        assert!(!is_placeholder_title("修复登录"));
    }
}

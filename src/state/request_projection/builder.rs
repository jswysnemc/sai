use super::estimate::project_provider_turn_estimate;
use super::memory_injection::memory_index_injection;
use super::model::{
    DynamicContextSource, ProjectedBaseContext, ProjectedRequest, ProjectedSessionSummary,
    ProjectionKind, ProjectionStats, ProjectionWarning,
};
use super::session_summary_projection::build_session_summary_projection_parts;
use super::validator::validate_provider_projection;
use crate::llm::ChatMessage;
use crate::state::{StateStore, StoredConversationEntry};
use anyhow::Result;

/// 从现有消息构造 provider turn 投影。
///
/// 参数:
/// - `messages`: 当前请求消息列表
/// - `tool_count`: 当前可见工具数量
/// - `context_limit_chars`: 当前模型上下文窗口字符数
///
/// 返回:
/// - provider turn 投影视图
pub(crate) fn project_provider_turn_from_messages(
    messages: &[ChatMessage],
    tool_count: usize,
    context_limit_chars: usize,
) -> ProjectedRequest {
    let mut projection = ProjectedRequest {
        kind: ProjectionKind::ProviderTurn,
        messages: messages.to_vec(),
        tool_count,
        estimate: project_provider_turn_estimate(messages, context_limit_chars),
        dynamic_sources: Vec::new(),
        provider_user_content: None,
        warnings: Vec::new(),
    };
    projection.warnings = validate_provider_projection(&projection);
    projection
}

/// 从基础上下文片段构造 provider base 消息。
///
/// 参数:
/// - `system_prompt`: 当前稳定 Context Epoch baseline
/// - `compaction_summary_context`: 会话压缩摘要上下文
/// - `history_entries`: 已持久化的历史消息入口
/// - `context_state_update`: 本轮需要持久化的状态变化
///
/// 返回:
/// - provider base 消息列表
#[allow(dead_code)]
pub(crate) fn project_provider_base_context(
    system_prompt: &str,
    compaction_summary_context: Option<&str>,
    history_entries: Vec<StoredConversationEntry>,
    context_state_update: Option<&str>,
) -> Vec<ChatMessage> {
    let mut projection = project_provider_base_context_projection(
        system_prompt,
        compaction_summary_context,
        entries_to_history_messages(history_entries),
        context_state_update,
    );
    let context = combine_provider_user_content(&projection.user_contexts, "");
    if !context.is_empty() {
        projection
            .messages
            .push(ChatMessage::plain("user", context));
    }
    projection.messages
}

/// 从基础上下文片段构造 provider base context 投影。
///
/// 参数:
/// - `system_prompt`: 当前稳定 Context Epoch baseline
/// - `compaction_summary_context`: 会话压缩摘要上下文
/// - `history_messages`: 已持久化的 provider 历史消息
/// - `context_state_update`: 本轮需要持久化的状态变化
///
/// 返回:
/// - provider base context 投影视图
pub(crate) fn project_provider_base_context_projection(
    system_prompt: &str,
    compaction_summary_context: Option<&str>,
    history_messages: Vec<ChatMessage>,
    context_state_update: Option<&str>,
) -> ProjectedBaseContext {
    let mut messages = vec![ChatMessage::system(system_prompt)];
    let mut dynamic_sources = Vec::new();
    let mut user_contexts = Vec::new();
    if let Some(summary) = compaction_summary_context {
        messages.push(ChatMessage::system(summary));
    }
    for message in history_messages {
        if message.role == "user" || message.role == "assistant" || message.role == "tool" {
            messages.push(message);
        }
    }
    // 1. 所有每轮可变上下文只追加在历史末端，保持系统与历史前缀逐字稳定
    if let Some(update) = context_state_update {
        dynamic_sources.push(dynamic_source("context_state_update", update));
        user_contexts.push(update.to_string());
    }
    ProjectedBaseContext {
        messages,
        dynamic_sources,
        user_contexts,
    }
}

/// 从基础上下文和当前用户输入构造 provider turn 投影。
///
/// 参数:
/// - `base_messages`: 已有系统、历史和运行时上下文消息
/// - `input`: 当前用户输入
/// - `image_urls`: 图片 data URL 列表
/// - `memory_index_prompt`: 可选记忆索引注入文本
/// - `auto_meme_reminder`: 可选自动表情包提醒
/// - `tool_count`: 当前可见工具数量
/// - `context_limit_chars`: 当前模型上下文窗口字符数
///
/// 返回:
/// - provider turn 投影视图
#[allow(dead_code)]
pub(crate) fn project_provider_turn_from_parts(
    base_messages: Vec<ChatMessage>,
    input: &str,
    image_url: Option<&str>,
    memory_index_prompt: Option<&str>,
    auto_meme_reminder: Option<&str>,
    tool_count: usize,
    context_limit_chars: usize,
) -> ProjectedRequest {
    let image_urls = image_url
        .map(|url| vec![url.to_string()])
        .unwrap_or_default();
    project_provider_turn_from_base_projection(
        ProjectedBaseContext {
            messages: base_messages,
            dynamic_sources: Vec::new(),
            user_contexts: Vec::new(),
        },
        input,
        &image_urls,
        memory_index_prompt,
        auto_meme_reminder,
        tool_count,
        context_limit_chars,
    )
}

/// 从基础上下文投影和当前用户输入构造 provider turn 投影。
///
/// 参数:
/// - `base_projection`: 已有系统、历史和运行时上下文投影
/// - `input`: 当前用户输入
/// - `image_urls`: 图片 data URL 列表
/// - `memory_index_prompt`: 可选记忆索引注入文本
/// - `auto_meme_reminder`: 可选自动表情包提醒
/// - `tool_count`: 当前可见工具数量
/// - `context_limit_chars`: 当前模型上下文窗口字符数
///
/// 返回:
/// - provider turn 投影视图
pub(crate) fn project_provider_turn_from_base_projection(
    base_projection: ProjectedBaseContext,
    input: &str,
    image_urls: &[String],
    memory_index_prompt: Option<&str>,
    auto_meme_reminder: Option<&str>,
    tool_count: usize,
    context_limit_chars: usize,
) -> ProjectedRequest {
    let mut base_messages = base_projection.messages;
    let mut dynamic_sources = base_projection.dynamic_sources;
    let mut user_contexts = base_projection.user_contexts;
    // 索引没变就跳过；本模型刚写入或其它进程改了则按差异/全文追加，详见 memory_injection
    if let Some(prompt) =
        memory_index_prompt.and_then(|prompt| memory_index_injection(&base_messages, prompt))
    {
        dynamic_sources.push(dynamic_source("memory_association", &prompt));
        user_contexts.push(prompt);
    }
    if let Some(reminder) = auto_meme_reminder {
        dynamic_sources.push(dynamic_source("auto_meme", reminder));
        user_contexts.push(reminder.to_string());
    }
    for (index, url) in image_urls.iter().enumerate() {
        dynamic_sources.push(dynamic_source(&format!("image_{}", index + 1), url));
    }
    let provider_user_content = combine_provider_user_content(&user_contexts, input);
    // 记忆召回与表情包提醒同样要落库：它们已经发给供应商，历史里缺失会让
    // 下一轮重放的用户消息与本轮实际发送内容不一致，前缀缓存从该消息断开
    let persisted_user_content = (!user_contexts.is_empty()).then(|| provider_user_content.clone());
    let user_message = if image_urls.is_empty() {
        ChatMessage::plain("user", provider_user_content)
    } else {
        ChatMessage::user_with_images(provider_user_content, image_urls.iter().cloned())
    };
    base_messages.push(user_message);
    let mut projection =
        project_provider_turn_from_messages(&base_messages, tool_count, context_limit_chars);
    projection.dynamic_sources = dynamic_sources;
    projection.provider_user_content = persisted_user_content;
    projection
}

/// 合并当前轮动态上下文与原始用户输入。
///
/// 参数:
/// - `contexts`: 当前轮内部上下文片段
/// - `input`: 原始用户输入
///
/// 返回:
/// - 供应商实际接收且可持久化重放的用户消息
fn combine_provider_user_content(contexts: &[String], input: &str) -> String {
    contexts
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(input))
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 构造动态上下文来源。
///
/// 参数:
/// - `key`: 动态来源 key
/// - `text`: 动态上下文文本
///
/// 返回:
/// - 动态上下文来源
fn dynamic_source(key: &str, text: &str) -> DynamicContextSource {
    DynamicContextSource {
        key: key.to_string(),
        chars: text.chars().count(),
    }
}

/// 将旧历史入口转换为 provider 历史消息。
///
/// 参数:
/// - `entries`: 旧历史入口列表
///
/// 返回:
/// - provider 历史消息列表
#[allow(dead_code)]
fn entries_to_history_messages(entries: Vec<StoredConversationEntry>) -> Vec<ChatMessage> {
    entries
        .into_iter()
        .filter(|entry| entry.role == "user" || entry.role == "assistant")
        .map(|entry| {
            // 历史思考随消息一并带出；是否真正发给供应商由请求组装阶段按开关决定
            let reasoning = entry.reasoning.clone();
            ChatMessage::plain(entry.role, entry.content).with_reasoning(reasoning)
        })
        .collect()
}

impl StateStore {
    /// 构造命令摘要投影视图。
    ///
    /// 参数:
    /// - `context_limit_chars`: 当前模型上下文窗口字符数
    ///
    /// 返回:
    /// - session summary 投影视图
    pub(crate) fn project_session_summary(
        &self,
        context_limit_chars: usize,
    ) -> Result<ProjectedSessionSummary> {
        let parts = build_session_summary_projection_parts(self, context_limit_chars)?;
        let warnings = validate_session_summary_projection(&parts.estimate, &parts.stats);
        Ok(ProjectedSessionSummary {
            kind: ProjectionKind::SessionSummary,
            estimate: parts.estimate,
            stats: parts.stats,
            compaction: parts.compaction,
            recovery: parts.recovery,
            warnings,
        })
    }
}

/// 校验命令摘要投影视图。
///
/// 参数:
/// - `estimate`: 摘要上下文估算
/// - `stats`: 摘要统计
///
/// 返回:
/// - 摘要投影警告列表
fn validate_session_summary_projection(
    estimate: &super::model::ProjectionEstimate,
    stats: &ProjectionStats,
) -> Vec<ProjectionWarning> {
    let mut warnings = Vec::new();
    if estimate.context_limit_chars == 0 {
        warnings.push(ProjectionWarning {
            message: "session summary projection has invalid context limit".to_string(),
        });
    }
    if stats.turn_count != stats.checkpoint_covered_turns + stats.tail_turns {
        warnings.push(ProjectionWarning {
            message: "session summary projection turn counts are inconsistent".to_string(),
        });
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatContent, ChatContentPart};
    use crate::state::StoredConversationEntry;

    fn text_content(message: &ChatMessage) -> String {
        match message.content.as_ref() {
            Some(ChatContent::Text(text)) => text.clone(),
            Some(ChatContent::Parts(parts)) => parts
                .iter()
                .filter_map(|part| match part {
                    ChatContentPart::Text { text } => Some(text.clone()),
                    ChatContentPart::ImageUrl { .. } => None,
                })
                .collect::<Vec<_>>()
                .join(""),
            None => String::new(),
        }
    }

    #[test]
    fn provider_turn_parts_append_dynamic_context_and_current_user() {
        let base = vec![
            ChatMessage::system("base"),
            ChatMessage::plain("user", "old user"),
            ChatMessage::plain("assistant", "old assistant"),
        ];

        let projection = project_provider_turn_from_parts(
            base,
            "current",
            Some("data:image/png;base64,abc"),
            Some("memory"),
            Some("meme"),
            2,
            1_000,
        );

        assert_eq!(projection.messages.len(), 4);
        assert_eq!(projection.messages[3].role, "user");
        assert_eq!(
            text_content(&projection.messages[3]),
            "memory\n\nmeme\n\ncurrent"
        );
        assert!(matches!(
            projection.messages[3].content.as_ref(),
            Some(ChatContent::Parts(parts)) if parts.len() == 2
        ));
        assert_eq!(projection.tool_count, 2);
        assert_eq!(projection.kind, ProjectionKind::ProviderTurn);
    }

    #[test]
    fn provider_turn_parts_records_dynamic_sources() {
        let projection = project_provider_turn_from_parts(
            vec![ChatMessage::system("base")],
            "current",
            None,
            Some("memory"),
            Some("meme"),
            0,
            1_000,
        );

        let sources = projection
            .dynamic_sources
            .iter()
            .map(|source| (source.key.as_str(), source.chars))
            .collect::<Vec<_>>();

        assert_eq!(sources, [("memory_association", 6), ("auto_meme", 4)]);
    }

    #[test]
    fn provider_turn_parts_records_image_dynamic_source() {
        let image_url = "data:image/png;base64,abc";
        let projection = project_provider_turn_from_parts(
            vec![ChatMessage::system("base")],
            "current",
            Some(image_url),
            None,
            None,
            0,
            1_000,
        );

        let sources = projection
            .dynamic_sources
            .iter()
            .map(|source| (source.key.as_str(), source.chars))
            .collect::<Vec<_>>();

        assert_eq!(sources, [("image_1", image_url.chars().count())]);
    }

    /// 【投影】【思考回传】验证历史思考随消息带出，供请求层按开关决定是否发送。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn history_messages_carry_previous_reasoning() {
        let entries = vec![
            StoredConversationEntry {
                timestamp: "1".to_string(),
                role: "user".to_string(),
                content: "question".to_string(),
                reasoning: None,
            },
            StoredConversationEntry {
                timestamp: "2".to_string(),
                role: "assistant".to_string(),
                content: "answer".to_string(),
                reasoning: Some("weighed two options".to_string()),
            },
        ];

        let messages = entries_to_history_messages(entries);

        assert_eq!(messages.len(), 2);
        assert!(messages[0].reasoning_content.is_none(), "用户消息没有思考");
        assert_eq!(
            messages[1].reasoning_content.as_deref(),
            Some("weighed two options")
        );
    }

    /// 【投影】【思考回传】验证空白思考不会占位。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn blank_reasoning_is_dropped() {
        let entries = vec![StoredConversationEntry {
            timestamp: "1".to_string(),
            role: "assistant".to_string(),
            content: "answer".to_string(),
            reasoning: Some("   ".to_string()),
        }];

        let messages = entries_to_history_messages(entries);

        assert!(messages[0].reasoning_content.is_none());
    }

    #[test]
    fn provider_base_context_matches_agent_message_order() {
        let history = vec![
            StoredConversationEntry {
                timestamp: "1".to_string(),
                role: "user".to_string(),
                content: "old user".to_string(),
                reasoning: None,
            },
            StoredConversationEntry {
                timestamp: "2".to_string(),
                role: "assistant".to_string(),
                content: "old assistant".to_string(),
                reasoning: None,
            },
            StoredConversationEntry {
                timestamp: "3".to_string(),
                role: "tool".to_string(),
                content: "ignored tool".to_string(),
                reasoning: None,
            },
        ];

        let messages =
            project_provider_base_context("system", Some("summary"), history, Some("runtime"));

        let roles = messages
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>();
        let texts = messages.iter().map(text_content).collect::<Vec<_>>();

        assert_eq!(roles, ["system", "system", "user", "assistant", "user"]);
        assert_eq!(
            texts,
            ["system", "summary", "old user", "old assistant", "runtime",]
        );
    }

    #[test]
    fn provider_base_context_records_dynamic_sources() {
        let projection = project_provider_base_context_projection(
            "system",
            Some("summary"),
            Vec::new(),
            Some("runtime"),
        );

        let sources = projection
            .dynamic_sources
            .iter()
            .map(|source| (source.key.as_str(), source.chars))
            .collect::<Vec<_>>();

        assert_eq!(sources, [("context_state_update", 7)]);
        assert_eq!(projection.user_contexts, ["runtime"]);
    }

    #[test]
    fn provider_base_context_preserves_tool_history_messages() {
        let assistant = ChatMessage::assistant(
            "",
            Some(vec![crate::llm::ToolCall {
                id: "call_1".to_string(),
                kind: "function".to_string(),
                function: crate::llm::ToolCallFunction {
                    name: "read_file".to_string(),
                    arguments: "{}".to_string(),
                },
            }]),
        );
        let projection = project_provider_base_context_projection(
            "system",
            None,
            vec![
                ChatMessage::plain("user", "inspect"),
                assistant,
                ChatMessage::tool("call_1", "content"),
                ChatMessage::plain("assistant", "done"),
            ],
            Some("runtime"),
        );

        let roles = projection
            .messages
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>();

        assert_eq!(roles, ["system", "user", "assistant", "tool", "assistant"]);
        assert!(projection.messages[2].tool_calls.is_some());
        assert_eq!(
            projection.messages[3].tool_call_id.as_deref(),
            Some("call_1")
        );
        assert_eq!(projection.user_contexts, ["runtime"]);
    }

    /// 【上下文缓存】【跨轮前缀】验证状态更新仅在变化轮次写入 provider 历史。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn state_update_persists_once_without_repeating_next_turn() {
        let first = project_provider_turn_from_base_projection(
            project_provider_base_context_projection(
                "system",
                None,
                Vec::new(),
                Some("runtime one"),
            ),
            "first request",
            &[],
            None,
            None,
            0,
            1_000,
        );
        assert_eq!(
            text_content(first.messages.last().unwrap()),
            "runtime one\n\nfirst request"
        );
        assert_eq!(
            first.provider_user_content.as_deref(),
            Some("runtime one\n\nfirst request")
        );
        let second = project_provider_turn_from_base_projection(
            project_provider_base_context_projection(
                "system",
                None,
                vec![
                    ChatMessage::plain("user", "runtime one\n\nfirst request"),
                    ChatMessage::plain("assistant", "first answer"),
                ],
                None,
            ),
            "second request",
            &[],
            None,
            None,
            0,
            1_000,
        );

        assert_eq!(
            text_content(&second.messages[1]),
            "runtime one\n\nfirst request"
        );
        assert_eq!(
            text_content(second.messages.last().unwrap()),
            "second request"
        );
        assert!(second.provider_user_content.is_none());
    }

    /// 【上下文缓存】【跨轮前缀】验证记忆召回与表情包提醒同样写入 provider 历史。
    ///
    /// 这两段内容已经发给供应商，若不落库，下一轮重放该轮用户消息时会缺失它们，
    /// 前缀与供应商缓存错位，该消息之后的全部历史都要重新计费。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn dynamic_user_contexts_are_persisted_verbatim() {
        let projection = project_provider_turn_from_base_projection(
            project_provider_base_context_projection("system", None, Vec::new(), Some("runtime")),
            "ask",
            &[],
            Some("<memory>fact</memory>"),
            Some("<system-reminder>meme</system-reminder>"),
            0,
            1_000,
        );

        let sent = text_content(projection.messages.last().unwrap());
        // 1. 落库内容必须与实际发送的用户消息逐字相同
        assert_eq!(
            projection.provider_user_content.as_deref(),
            Some(sent.as_str())
        );
        assert_eq!(
            sent,
            "runtime\n\n<memory>fact</memory>\n\n<system-reminder>meme</system-reminder>\n\nask"
        );

        // 2. 下一轮以落库内容重放时，该消息与本轮发送内容一致
        let next = project_provider_turn_from_base_projection(
            project_provider_base_context_projection(
                "system",
                None,
                vec![
                    ChatMessage::plain("user", sent.clone()),
                    ChatMessage::plain("assistant", "answer"),
                ],
                None,
            ),
            "next",
            &[],
            None,
            None,
            0,
            1_000,
        );
        assert_eq!(text_content(&next.messages[1]), sent);
    }
}

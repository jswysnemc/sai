use super::estimate::project_provider_turn_estimate;
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
        warnings: Vec::new(),
    };
    projection.warnings = validate_provider_projection(&projection);
    projection
}

/// 从基础上下文片段构造 provider base 消息。
///
/// 参数:
/// - `system_prompt`: 当前稳定 Context Epoch baseline
/// - `mode_reminder`: 当前模式提醒
/// - `selected_model`: 当前 provider/model 标签
/// - `session_goal_context`: 当前持续目标上下文
/// - `compaction_summary_context`: 会话压缩摘要上下文
/// - `history_entries`: 已持久化的历史消息入口
/// - `last_auto_meme_reminder`: 最近一次自动表情包提醒
/// - `runtime_context`: 当前运行时上下文
///
/// 返回:
/// - provider base 消息列表
#[allow(dead_code)]
pub(crate) fn project_provider_base_context(
    system_prompt: &str,
    mode_reminder: Option<&str>,
    selected_model: Option<&str>,
    session_goal_context: Option<&str>,
    compaction_summary_context: Option<&str>,
    history_entries: Vec<StoredConversationEntry>,
    last_auto_meme_reminder: Option<&str>,
    runtime_context: &str,
) -> Vec<ChatMessage> {
    let mut projection = project_provider_base_context_projection(
        system_prompt,
        mode_reminder,
        selected_model,
        session_goal_context,
        compaction_summary_context,
        entries_to_history_messages(history_entries),
        last_auto_meme_reminder,
        runtime_context,
    );
    let context = combine_provider_user_content(&projection.pending_user_contexts, "");
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
/// - `mode_reminder`: 当前模式提醒
/// - `selected_model`: 当前 provider/model 标签
/// - `session_goal_context`: 当前持续目标上下文
/// - `compaction_summary_context`: 会话压缩摘要上下文
/// - `history_messages`: 已持久化的 provider 历史消息
/// - `last_auto_meme_reminder`: 最近一次自动表情包提醒
/// - `runtime_context`: 当前运行时上下文
///
/// 返回:
/// - provider base context 投影视图
pub(crate) fn project_provider_base_context_projection(
    system_prompt: &str,
    mode_reminder: Option<&str>,
    selected_model: Option<&str>,
    session_goal_context: Option<&str>,
    compaction_summary_context: Option<&str>,
    history_messages: Vec<ChatMessage>,
    last_auto_meme_reminder: Option<&str>,
    runtime_context: &str,
) -> ProjectedBaseContext {
    let mut messages = vec![ChatMessage::system(system_prompt)];
    let mut dynamic_sources = Vec::new();
    let mut pending_user_contexts = Vec::new();
    if let Some(reminder) = mode_reminder {
        dynamic_sources.push(dynamic_source("mode_reminder", reminder));
    }
    if let Some(model) = selected_model {
        dynamic_sources.push(dynamic_source("selected_model", model));
    }
    if let Some(summary) = compaction_summary_context {
        messages.push(ChatMessage::system(summary));
    }
    for message in history_messages {
        if message.role == "user" || message.role == "assistant" || message.role == "tool" {
            messages.push(message);
        }
    }
    // 1. 所有每轮可变上下文只追加在历史末端，保持系统与历史前缀逐字稳定
    if let Some(reminder) = mode_reminder {
        pending_user_contexts.push(reminder.to_string());
    }
    if let Some(prompt) = session_goal_context {
        dynamic_sources.push(dynamic_source("session_goal", prompt));
        pending_user_contexts.push(prompt.to_string());
    }
    if let Some(reminder) = last_auto_meme_reminder {
        dynamic_sources.push(dynamic_source("last_auto_meme", reminder));
        pending_user_contexts.push(reminder.to_string());
    }
    dynamic_sources.push(dynamic_source("runtime_context", runtime_context));
    pending_user_contexts.push(runtime_context.to_string());
    ProjectedBaseContext {
        messages,
        dynamic_sources,
        pending_user_contexts,
    }
}

/// 从基础上下文和当前用户输入构造 provider turn 投影。
///
/// 参数:
/// - `base_messages`: 已有系统、历史和运行时上下文消息
/// - `input`: 当前用户输入
/// - `image_urls`: 图片 data URL 列表
/// - `association_prompt`: 可选关联记忆上下文
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
    association_prompt: Option<&str>,
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
            pending_user_contexts: Vec::new(),
        },
        input,
        &image_urls,
        association_prompt,
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
/// - `association_prompt`: 可选关联记忆上下文
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
    association_prompt: Option<&str>,
    auto_meme_reminder: Option<&str>,
    tool_count: usize,
    context_limit_chars: usize,
) -> ProjectedRequest {
    let mut base_messages = base_projection.messages;
    let mut dynamic_sources = base_projection.dynamic_sources;
    let mut pending_user_contexts = base_projection.pending_user_contexts;
    if let Some(prompt) = association_prompt {
        dynamic_sources.push(dynamic_source("memory_association", prompt));
        pending_user_contexts.push(prompt.to_string());
    }
    if let Some(reminder) = auto_meme_reminder {
        dynamic_sources.push(dynamic_source("auto_meme", reminder));
        pending_user_contexts.push(reminder.to_string());
    }
    for (index, url) in image_urls.iter().enumerate() {
        dynamic_sources.push(dynamic_source(&format!("image_{}", index + 1), url));
    }
    let provider_user_content = combine_provider_user_content(&pending_user_contexts, input);
    let user_message = if image_urls.is_empty() {
        ChatMessage::plain("user", provider_user_content)
    } else {
        ChatMessage::user_with_images(provider_user_content, image_urls.iter().cloned())
    };
    base_messages.push(user_message);
    let mut projection =
        project_provider_turn_from_messages(&base_messages, tool_count, context_limit_chars);
    projection.dynamic_sources = dynamic_sources;
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

        let messages = project_provider_base_context(
            "system",
            Some("mode"),
            None,
            Some("loaded tools"),
            Some("summary"),
            history,
            Some("last meme"),
            "runtime",
        );

        let roles = messages
            .iter()
            .map(|message| message.role.as_str())
            .collect::<Vec<_>>();
        let texts = messages.iter().map(text_content).collect::<Vec<_>>();

        assert_eq!(roles, ["system", "system", "user", "assistant", "user"]);
        assert_eq!(
            texts,
            [
                "system",
                "summary",
                "old user",
                "old assistant",
                "mode\n\nloaded tools\n\nlast meme\n\nruntime",
            ]
        );
    }

    #[test]
    fn provider_base_context_records_dynamic_sources() {
        let projection = project_provider_base_context_projection(
            "system",
            Some("mode"),
            Some("provider/model"),
            Some("loaded tools"),
            Some("summary"),
            Vec::new(),
            Some("last meme"),
            "runtime",
        );

        let sources = projection
            .dynamic_sources
            .iter()
            .map(|source| (source.key.as_str(), source.chars))
            .collect::<Vec<_>>();

        assert_eq!(
            sources,
            [
                ("mode_reminder", 4),
                ("selected_model", 14),
                ("session_goal", 12),
                ("last_auto_meme", 9),
                ("runtime_context", 7),
            ]
        );
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
            None,
            None,
            None,
            vec![
                ChatMessage::plain("user", "inspect"),
                assistant,
                ChatMessage::tool("call_1", "content"),
                ChatMessage::plain("assistant", "done"),
            ],
            None,
            "runtime",
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
        assert_eq!(projection.pending_user_contexts, ["runtime"]);
    }

    #[test]
    fn changing_goal_progress_preserves_the_stable_system_prefix() {
        let project = |goal: &str| {
            project_provider_base_context_projection(
                "system",
                Some("mode"),
                None,
                Some(goal),
                Some("summary"),
                vec![
                    ChatMessage::plain("user", "old input"),
                    ChatMessage::plain("assistant", "old reply"),
                ],
                None,
                "runtime",
            )
        };
        let first = project("tokens used: 10");
        let second = project("tokens used: 20");

        assert_eq!(
            serde_json::to_value(&first.messages).unwrap(),
            serde_json::to_value(&second.messages).unwrap()
        );
        assert_ne!(first.pending_user_contexts, second.pending_user_contexts);
    }

    /// 【上下文缓存】【跨轮前缀】验证上一轮实际用户消息可成为下一轮稳定前缀。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn persisted_provider_input_preserves_cross_turn_prefix() {
        let first = project_provider_turn_from_base_projection(
            project_provider_base_context_projection(
                "system",
                Some("mode"),
                None,
                None,
                None,
                Vec::new(),
                None,
                "runtime one",
            ),
            "first request",
            &[],
            None,
            None,
            0,
            1_000,
        );
        let first_provider_input = text_content(first.messages.last().unwrap());
        let second = project_provider_turn_from_base_projection(
            project_provider_base_context_projection(
                "system",
                Some("mode"),
                None,
                None,
                None,
                vec![
                    ChatMessage::plain("user", first_provider_input),
                    ChatMessage::plain("assistant", "first answer"),
                ],
                None,
                "runtime two",
            ),
            "second request",
            &[],
            None,
            None,
            0,
            1_000,
        );

        assert_eq!(
            serde_json::to_value(&first.messages).unwrap(),
            serde_json::to_value(&second.messages[..first.messages.len()]).unwrap()
        );
    }
}

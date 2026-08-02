use crate::llm::{ChatContent, ChatMessage};

/// 压缩摘要消息的可见前缀标记。
///
/// 压缩后重建历史时用它识别上一条摘要，避免摘要逐次嵌套。
pub(crate) const COMPACTION_SUMMARY_MARKER: &str = "<conversation-handoff>";

/// 省略标记的可见前缀。
///
/// 用户消息池超出预算时插在头尾两段之间，说明中间有内容被摘要覆盖。
pub(crate) const COMPACTION_ELISION_MARKER: &str = "<omitted-user-messages";

/// 运行期注入内容的可见前缀。
///
/// 这些消息在压缩后由各自的注入器重新生成，保留下来只会与新注入的内容重复。
const RUNTIME_INJECTION_MARKERS: &[&str] = &[
    "<runtime-context>",
    "<external-completion-events>",
    "<turn_aborted>",
    "<system-reminder>",
    "<todo-reminder>",
    "<goal-status>",
];

/// 用户消息在压缩后的处置方式。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum UserMessageDisposition {
    /// 真实用户输入，逐字保留
    Keep,
    /// 运行期注入或压缩产物，压缩后丢弃
    Drop,
}

/// 判定 user 角色消息在压缩后是否保留。
///
/// sai 的 `ChatMessage` 没有来源标记字段，只能按可见前缀识别。运行期注入的
/// 上下文、外部事件回执、中断标记等虽然也是 user 角色，但压缩后会由各自的
/// 注入路径重新生成，保留下来既重复又会挤占真实用户输入的预算。
///
/// 参数:
/// - `message`: 待判定消息
///
/// 返回:
/// - 保留或丢弃
pub(crate) fn user_message_disposition(message: &ChatMessage) -> UserMessageDisposition {
    if message.role != "user" {
        return UserMessageDisposition::Drop;
    }
    let Some(text) = message_text(message) else {
        // 纯图片消息没有文本，按真实用户输入处理
        return UserMessageDisposition::Keep;
    };
    let trimmed = text.trim_start();
    // 1. 上一轮摘要与省略标记不再参与保留，否则会逐次嵌套
    if trimmed.starts_with(COMPACTION_SUMMARY_MARKER)
        || trimmed.starts_with(COMPACTION_ELISION_MARKER)
    {
        return UserMessageDisposition::Drop;
    }
    // 2. 运行期注入内容压缩后重新生成
    if RUNTIME_INJECTION_MARKERS
        .iter()
        .any(|marker| trimmed.starts_with(marker))
    {
        return UserMessageDisposition::Drop;
    }
    UserMessageDisposition::Keep
}

/// 判断消息是否为需要逐字保留的真实用户输入。
///
/// 参数:
/// - `message`: 待判定消息
///
/// 返回:
/// - 是否为真实用户输入
pub(crate) fn is_real_user_input(message: &ChatMessage) -> bool {
    user_message_disposition(message) == UserMessageDisposition::Keep
}

/// 提取消息的纯文本内容。
///
/// 参数:
/// - `message`: 待提取消息
///
/// 返回:
/// - 文本内容；多模态消息拼接其中的文本片段，无文本时为 None
pub(crate) fn message_text(message: &ChatMessage) -> Option<String> {
    match message.content.as_ref()? {
        ChatContent::Text(text) => Some(text.clone()),
        ChatContent::Parts(parts) => {
            let text = parts
                .iter()
                .filter_map(|part| match part {
                    crate::llm::ChatContentPart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
    }
}

/// 用新文本替换消息正文，保留其余字段。
///
/// 参数:
/// - `message`: 原始消息
/// - `text`: 新的文本内容
///
/// 返回:
/// - 替换正文后的消息
pub(crate) fn replace_message_text(message: &ChatMessage, text: String) -> ChatMessage {
    ChatMessage {
        role: message.role.clone(),
        content: Some(ChatContent::Text(text)),
        tool_call_id: message.tool_call_id.clone(),
        tool_calls: message.tool_calls.clone(),
        reasoning_content: message.reasoning_content.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造指定正文的 user 消息。
    ///
    /// 参数:
    /// - `text`: 消息正文
    ///
    /// 返回:
    /// - user 角色消息
    fn user(text: &str) -> ChatMessage {
        ChatMessage::plain("user", text.to_string())
    }

    /// 验证真实用户输入被保留。
    #[test]
    fn keeps_real_user_input() {
        assert!(is_real_user_input(&user("帮我重构这个模块")));
    }

    /// 验证上一轮摘要不再参与保留。
    #[test]
    fn drops_previous_summary() {
        let summary = user(&format!("{COMPACTION_SUMMARY_MARKER}\n之前的交接笔记"));

        assert!(!is_real_user_input(&summary));
    }

    /// 验证省略标记不会逐次堆叠。
    #[test]
    fn drops_elision_marker() {
        let marker = user(&format!("{COMPACTION_ELISION_MARKER} count=\"3\" />"));

        assert!(!is_real_user_input(&marker));
    }

    /// 验证运行期注入内容压缩后丢弃。
    #[test]
    fn drops_runtime_injections() {
        for marker in RUNTIME_INJECTION_MARKERS {
            let message = user(&format!("{marker}\n注入内容"));

            assert!(
                !is_real_user_input(&message),
                "{marker} must not survive compaction"
            );
        }
    }

    /// 验证非 user 角色一律不进入保留集合。
    #[test]
    fn drops_non_user_roles() {
        assert!(!is_real_user_input(&ChatMessage::plain(
            "assistant",
            "回复".to_string()
        )));
        assert!(!is_real_user_input(&ChatMessage::tool(
            "call_1".to_string(),
            "结果".to_string()
        )));
    }

    /// 验证前导空白不影响标记识别。
    #[test]
    fn recognizes_markers_after_leading_whitespace() {
        let summary = user(&format!("\n  {COMPACTION_SUMMARY_MARKER}\n笔记"));

        assert!(!is_real_user_input(&summary));
    }

    /// 验证替换正文时保留其余字段。
    #[test]
    fn replace_text_preserves_other_fields() {
        let mut message = user("原始");
        message.reasoning_content = Some("思考".to_string());

        let replaced = replace_message_text(&message, "新的".to_string());

        assert_eq!(message_text(&replaced).as_deref(), Some("新的"));
        assert_eq!(replaced.reasoning_content.as_deref(), Some("思考"));
        assert_eq!(replaced.role, "user");
    }
}

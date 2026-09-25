use crate::llm::ChatMessage;
use crate::tools::model_attachment::attachment_message;
use crate::tools::ToolModelAttachment;

/// 把同一工具子轮返回的图片追加到上下文。
///
/// 附件消息与工具结果一样保留在后续请求里，模型可以在整轮中反复对照图片；
/// 历史投影会从持久化记录重建同样的消息。
///
/// 参数:
/// - `messages`: 当前模型上下文消息
/// - `attachments`: 本子轮工具返回的图片附件
///
/// 返回:
/// - 无
pub(super) fn append_model_attachments(
    messages: &mut Vec<ChatMessage>,
    attachments: Vec<ToolModelAttachment>,
) {
    if let Some(message) = attachment_message(&attachments) {
        messages.push(message);
    }
}

#[cfg(test)]
mod tests {
    use super::append_model_attachments;
    use crate::llm::ChatMessage;
    use crate::tools::model_attachment::is_attachment_message;
    use crate::tools::ToolModelAttachment;

    /// 附件追加后保留在上下文中，不会在请求后被移除。
    #[test]
    fn appended_images_stay_in_context() {
        let mut messages = vec![ChatMessage::plain("user", "检查图片")];
        append_model_attachments(
            &mut messages,
            vec![ToolModelAttachment::new(
                "data:image/png;base64,AA",
                "a.png",
                "[Image: source: a.png]",
            )],
        );

        assert_eq!(messages.len(), 2);
        assert!(is_attachment_message(&messages[1]));
    }

    /// 没有附件时不追加空消息。
    #[test]
    fn empty_round_adds_nothing() {
        let mut messages = vec![ChatMessage::plain("user", "hi")];
        append_model_attachments(&mut messages, Vec::new());
        assert_eq!(messages.len(), 1);
    }
}

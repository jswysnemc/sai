use crate::llm::ChatMessage;

/// 工具附件消息的起始标记。
const TOOL_IMAGE_MESSAGE_OPEN: &str = "<tool-image-attachments>";
/// 工具附件消息的结束标记。
const TOOL_IMAGE_MESSAGE_CLOSE: &str = "</tool-image-attachments>";

/// 工具结果携带、需要以图片形式交给当前模型的附件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolModelAttachment {
    /// 图片 data URL 或远程 URL
    pub(crate) image_url: String,
    /// 图片来源路径或标识
    pub(crate) source: String,
    /// 与工具结果文本一致的图片说明（来源、格式、尺寸、缩放比例）
    pub(crate) note: String,
}

impl ToolModelAttachment {
    /// 创建模型图片附件。
    ///
    /// 参数:
    /// - `image_url`: 图片 data URL 或远程 URL
    /// - `source`: 图片来源路径或标识
    /// - `note`: 图片说明
    ///
    /// 返回:
    /// - 模型图片附件
    pub(crate) fn new(
        image_url: impl Into<String>,
        source: impl Into<String>,
        note: impl Into<String>,
    ) -> Self {
        Self {
            image_url: image_url.into(),
            source: source.into(),
            note: note.into(),
        }
    }
}

/// 把同一工具子轮的图片合并成一条紧跟工具结果的用户消息。
///
/// OpenAI 兼容接口的 tool 消息只能携带文本，图片只能放进随后的用户消息；
/// 这条消息与工具结果一起留在上下文里，并由会话历史投影原样重建。
///
/// 参数:
/// - `attachments`: 按调用顺序排列的图片附件
///
/// 返回:
/// - 多模态用户消息；没有附件时为 None
pub(crate) fn attachment_message(attachments: &[ToolModelAttachment]) -> Option<ChatMessage> {
    if attachments.is_empty() {
        return None;
    }
    let mut text = String::from(TOOL_IMAGE_MESSAGE_OPEN);
    text.push_str("\nImages returned by the tool calls above, attached in order:\n");
    for (index, attachment) in attachments.iter().enumerate() {
        text.push_str(&format!("{}. {}\n", index + 1, attachment.note));
    }
    text.push_str(TOOL_IMAGE_MESSAGE_CLOSE);
    Some(ChatMessage::user_with_images(
        text,
        attachments
            .iter()
            .map(|attachment| attachment.image_url.clone()),
    ))
}

/// 判断消息是否为工具图片附件消息。
///
/// 参数:
/// - `message`: 待检查消息
///
/// 返回:
/// - 首段文本带附件标记时为 true
#[cfg(test)]
pub(crate) fn is_attachment_message(message: &ChatMessage) -> bool {
    use crate::llm::{ChatContent, ChatContentPart};
    let Some(ChatContent::Parts(parts)) = message.content.as_ref() else {
        return false;
    };
    parts.iter().any(|part| {
        matches!(part, ChatContentPart::Text { text } if text.starts_with(TOOL_IMAGE_MESSAGE_OPEN))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatContent, ChatContentPart};

    /// 多张工具图片合并为一条多模态消息，说明按顺序编号。
    #[test]
    fn merges_round_images_into_one_message() {
        let message = attachment_message(&[
            ToolModelAttachment::new(
                "data:image/png;base64,AA",
                "a.png",
                "[Image: source: a.png]",
            ),
            ToolModelAttachment::new(
                "data:image/png;base64,BB",
                "b.png",
                "[Image: source: b.png]",
            ),
        ])
        .unwrap();

        assert!(is_attachment_message(&message));
        let Some(ChatContent::Parts(parts)) = message.content.as_ref() else {
            panic!("expected multimodal message");
        };
        assert_eq!(
            parts
                .iter()
                .filter(|part| matches!(part, ChatContentPart::ImageUrl { .. }))
                .count(),
            2
        );
        let ChatContentPart::Text { text } = &parts[0] else {
            panic!("expected leading text");
        };
        assert!(text.contains("1. [Image: source: a.png]"));
        assert!(text.contains("2. [Image: source: b.png]"));
    }

    /// 没有附件时不生成消息，普通用户消息不会被误判。
    #[test]
    fn plain_messages_are_not_attachments() {
        assert!(attachment_message(&[]).is_none());
        assert!(!is_attachment_message(&ChatMessage::plain("user", "hello")));
    }
}

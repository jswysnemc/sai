use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use crate::tools::ToolModelAttachment;

const TOOL_IMAGE_MESSAGE_OPEN: &str = "<tool-image-attachments>";
const TOOL_IMAGE_MESSAGE_CLOSE: &str = "</tool-image-attachments>";

/// 将同一工具批次中的图片合并为一次临时多模态消息。
///
/// 参数:
/// - `messages`: 当前模型上下文消息
/// - `attachments`: 工具返回的临时图片附件
///
/// 返回:
/// - 无
pub(super) fn append_model_attachments(
    messages: &mut Vec<ChatMessage>,
    attachments: Vec<ToolModelAttachment>,
) {
    if attachments.is_empty() {
        return;
    }
    let mut prompt = String::from(TOOL_IMAGE_MESSAGE_OPEN);
    prompt.push_str("\nImages returned by read_file are attached for direct analysis. ");
    prompt.push_str("Follow each associated prompt and continue the current tool task.\n");
    for (index, attachment) in attachments.iter().enumerate() {
        prompt.push_str(&format!(
            "{}. source: {}\n   prompt: {}\n",
            index + 1,
            attachment.source,
            attachment.prompt
        ));
    }
    prompt.push_str(TOOL_IMAGE_MESSAGE_CLOSE);
    messages.push(ChatMessage::user_with_images(
        prompt,
        attachments
            .into_iter()
            .map(|attachment| attachment.image_url),
    ));
}

/// 判断消息是否为等待下一次模型请求消费的工具图片附件。
///
/// 参数:
/// - `message`: 待检查消息
///
/// 返回:
/// - 包含内部附件标记时返回 true
pub(super) fn is_pending_model_attachment(message: &ChatMessage) -> bool {
    let Some(ChatContent::Parts(parts)) = message.content.as_ref() else {
        return false;
    };
    parts.iter().any(|part| {
        matches!(part, ChatContentPart::Text { text } if text.starts_with(TOOL_IMAGE_MESSAGE_OPEN))
    })
}

/// 在模型成功消费图片后移除临时附件消息。
///
/// 参数:
/// - `messages`: 当前模型上下文消息
///
/// 返回:
/// - 无
pub(super) fn remove_pending_model_attachments(messages: &mut Vec<ChatMessage>) {
    messages.retain(|message| !is_pending_model_attachment(message));
}

#[cfg(test)]
mod tests {
    use super::{
        append_model_attachments, is_pending_model_attachment, remove_pending_model_attachments,
    };
    use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
    use crate::tools::ToolModelAttachment;

    /// 多张工具图片会合并为一次多模态附件提交。
    #[test]
    fn merges_tool_images_into_one_model_message() {
        let mut messages = vec![ChatMessage::plain("user", "检查图片")];
        append_model_attachments(
            &mut messages,
            vec![
                ToolModelAttachment::new("data:image/png;base64,AA", "a.png", "读取文字"),
                ToolModelAttachment::new("data:image/png;base64,BB", "b.png", "比较颜色"),
            ],
        );

        assert_eq!(messages.len(), 2);
        assert!(is_pending_model_attachment(&messages[1]));
        let Some(ChatContent::Parts(parts)) = messages[1].content.as_ref() else {
            panic!("expected multimodal message");
        };
        assert_eq!(
            parts
                .iter()
                .filter(|part| matches!(part, ChatContentPart::ImageUrl { .. }))
                .count(),
            2
        );
    }

    /// 模型请求完成后临时 data URL 不再进入后续请求。
    #[test]
    fn removes_model_attachment_after_single_submission() {
        let mut messages = vec![ChatMessage::plain("user", "检查图片")];
        append_model_attachments(
            &mut messages,
            vec![ToolModelAttachment::new(
                "data:image/png;base64,AA",
                "a.png",
                "读取文字",
            )],
        );

        remove_pending_model_attachments(&mut messages);

        assert_eq!(messages.len(), 1);
        assert!(!messages.iter().any(is_pending_model_attachment));
    }
}

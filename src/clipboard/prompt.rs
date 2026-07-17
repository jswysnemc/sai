use super::payload::{ClipboardChatInput, ClipboardPayload};

const DEFAULT_IMAGE_PROMPT: &str = "请根据剪贴板图片回答。";

/// 将剪贴板内容应用到当前用户提示词。
///
/// 参数:
/// - `message`: 用户输入提示词
/// - `payload`: 剪贴板载荷
///
/// 返回:
/// - 注入后的提示词和可选图片 URL
pub fn apply_clipboard_payload(message: String, payload: ClipboardPayload) -> ClipboardChatInput {
    match payload {
        ClipboardPayload::Text(text) => ClipboardChatInput {
            message: inject_clipboard_text(&message, &text),
            image_url: None,
        },
        ClipboardPayload::ImageDataUrl { data_url, .. } => ClipboardChatInput {
            message: image_prompt(&message),
            image_url: Some(data_url),
        },
    }
}

/// 将剪贴板文本注入用户提示词。
///
/// 参数:
/// - `message`: 用户输入提示词
/// - `clipboard_text`: 剪贴板文本
///
/// 返回:
/// - 注入剪贴板文本后的提示词
fn inject_clipboard_text(message: &str, clipboard_text: &str) -> String {
    let message = message.trim();
    let clipboard_text = clipboard_text.trim();
    if message.is_empty() {
        return clipboard_text.to_string();
    }
    format!("{message}\n\n<clipboard>\n{clipboard_text}\n</clipboard>")
}

/// 生成带剪贴板图片的提示词。
///
/// 参数:
/// - `message`: 用户输入提示词
///
/// 返回:
/// - 非空图片提示词
fn image_prompt(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        DEFAULT_IMAGE_PROMPT.to_string()
    } else {
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_clipboard_text_into_prompt() {
        let input = apply_clipboard_payload(
            "总结".to_string(),
            ClipboardPayload::Text("内容".to_string()),
        );
        assert_eq!(input.message, "总结\n\n<clipboard>\n内容\n</clipboard>");
        assert!(input.image_url.is_none());
    }

    #[test]
    fn uses_clipboard_text_as_prompt_when_message_is_empty() {
        let input =
            apply_clipboard_payload(String::new(), ClipboardPayload::Text("内容".to_string()));
        assert_eq!(input.message, "内容");
        assert!(input.image_url.is_none());
    }

    #[test]
    fn attaches_image_url_without_injecting_text() {
        let input = apply_clipboard_payload(
            "描述图片".to_string(),
            ClipboardPayload::ImageDataUrl {
                data_url: "data:image/png;base64,abc".to_string(),
                width: 1,
                height: 1,
            },
        );
        assert_eq!(input.message, "描述图片");
        assert_eq!(
            input.image_url.as_deref(),
            Some("data:image/png;base64,abc")
        );
    }
}

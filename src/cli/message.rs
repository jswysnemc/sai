use super::*;

pub(super) fn join_message(parts: Vec<String>) -> String {
    parts.join(" ").trim().to_string()
}

/// 准备带剪贴板内容的聊天输入。
///
/// 参数:
/// - `message`: 用户消息
/// - `clipb`: 是否启用剪贴板注入
///
/// 返回:
/// - 最终文本消息和可选图片 URL
pub(super) fn prepare_clipboard_chat_input(
    message: String,
    clipb: bool,
) -> Result<clipboard::ClipboardChatInput> {
    if !clipb {
        return Ok(clipboard::ClipboardChatInput {
            message,
            image_url: None,
        });
    }
    let payload = clipboard::read_clipboard_payload()?;
    Ok(clipboard::apply_clipboard_payload(message, payload))
}

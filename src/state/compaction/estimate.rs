use crate::llm::{ChatContent, ChatContentPart, ChatMessage, Usage};
use crate::token_estimate;
use base64::Engine;

/// 单张图片在缺少像素信息时的保守视觉 token。
const FALLBACK_IMAGE_TOKENS: usize = 2_048;
/// 图片估算上限，避免异常尺寸把占用顶满。
const MAX_IMAGE_TOKENS: usize = 8_192;
/// 每条消息的协议开销（role、分隔符），不把整包 JSON 再送进分词器。
const MESSAGE_OVERHEAD_TOKENS: usize = 6;

/// 估算即将发送给模型的消息上下文字符数（兼容旧路径）。
///
/// 参数:
/// - `messages`: 当前请求消息列表
///
/// 返回:
/// - JSON 序列化后的字符数量估算
pub fn estimate_chat_messages_chars(messages: &[ChatMessage]) -> usize {
    serde_json::to_string(messages)
        .map(|value| value.chars().count())
        .unwrap_or_else(|_| {
            messages
                .iter()
                .map(|message| format!("{message:?}").chars().count())
                .sum()
        })
}

/// 估算即将发送给模型的消息上下文 token 数。
///
/// 按消息部件分别估算：文本走 o200k_base，图片按视觉公式，
/// 不把 data URL 的 base64 当作文本送进分词器。
///
/// 参数:
/// - `messages`: 当前请求消息列表
///
/// 返回:
/// - 文本 BPE 与图片视觉 token 之和
pub fn estimate_chat_messages_tokens(messages: &[ChatMessage]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

/// 计算压缩触发用的上下文占用。
///
/// API 回报了 prompt_tokens 时：上次 prompt + 上次 completion + 尚未计入的尾部消息。
/// 未回报时：对当前整包消息做图片友好的全量预估。
///
/// 参数:
/// - `messages`: 即将发给模型的消息
/// - `last_usage`: 最近一次主对话 provider 用量
///
/// 返回:
/// - 用于和窗口预算比较的占用 token
pub fn occupancy_tokens(messages: &[ChatMessage], last_usage: Option<&Usage>) -> usize {
    match last_usage.filter(|usage| usage.prompt_tokens > 0) {
        Some(usage) => (usage.prompt_tokens as usize)
            .saturating_add(usage.completion_tokens as usize)
            .saturating_add(estimate_unsent_tail(messages)),
        None => estimate_chat_messages_tokens(messages),
    }
}

/// 估算单条消息的 token。
///
/// 参数:
/// - `message`: 一条聊天消息
///
/// 返回:
/// - 该消息的估算 token
fn estimate_message_tokens(message: &ChatMessage) -> usize {
    let mut tokens = MESSAGE_OVERHEAD_TOKENS;
    tokens += token_estimate::estimate_tokens(&message.role);
    tokens += estimate_content_tokens(message.content.as_ref());
    if let Some(reasoning) = message.reasoning_content.as_deref() {
        tokens = tokens.saturating_add(token_estimate::estimate_tokens(reasoning));
    }
    if let Some(tool_calls) = message.tool_calls.as_ref() {
        for call in tool_calls {
            tokens = tokens.saturating_add(token_estimate::estimate_tokens(&call.function.name));
            tokens =
                tokens.saturating_add(token_estimate::estimate_tokens(&call.function.arguments));
        }
    }
    tokens
}

/// 估算消息正文；图片走视觉口径。
///
/// 参数:
/// - `content`: 消息正文
///
/// 返回:
/// - 正文 token
fn estimate_content_tokens(content: Option<&ChatContent>) -> usize {
    match content {
        Some(ChatContent::Text(text)) => token_estimate::estimate_tokens(text),
        Some(ChatContent::Parts(parts)) => parts
            .iter()
            .map(|part| match part {
                ChatContentPart::Text { text } => token_estimate::estimate_tokens(text),
                ChatContentPart::ImageUrl { image_url } => {
                    estimate_image_url_tokens(&image_url.url)
                }
            })
            .sum(),
        None => 0,
    }
}

/// 估算尚未被上次 API usage 计入的尾部消息。
///
/// 从末尾收集连续的 tool 结果；若最后一条是新的 user，则只计这一条。
/// 轮次开头对应新用户输入，工具轮次对应刚返回的 tool 结果。
///
/// 参数:
/// - `messages`: 即将发出的完整消息
///
/// 返回:
/// - 尾部增量 token
fn estimate_unsent_tail(messages: &[ChatMessage]) -> usize {
    let mut tail = Vec::new();
    for message in messages.iter().rev() {
        match message.role.as_str() {
            "tool" => tail.push(message),
            "user" => {
                tail.push(message);
                break;
            }
            _ => break,
        }
    }
    tail.into_iter().rev().map(estimate_message_tokens).sum()
}

/// 估算一张图片的视觉 token。
///
/// data URL 按像素分块；普通 http URL 只计短文本；解不出尺寸时用保守常量。
///
/// 参数:
/// - `url`: 图片地址或 data URL
///
/// 返回:
/// - 视觉 token，不超过上限
fn estimate_image_url_tokens(url: &str) -> usize {
    if let Some(tokens) = data_url_image_tokens(url) {
        return tokens.min(MAX_IMAGE_TOKENS);
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        return token_estimate::estimate_tokens(url).max(85);
    }
    FALLBACK_IMAGE_TOKENS
}

/// 从 data URL 解码尺寸并按 512 分块估算。
///
/// 参数:
/// - `url`: data URL
///
/// 返回:
/// - 解出尺寸时的视觉 token
fn data_url_image_tokens(url: &str) -> Option<usize> {
    let payload = url.strip_prefix("data:")?;
    let (_header, encoded) = payload.split_once(',')?;
    let bytes = decode_base64_prefix(encoded, 32)?;
    let (width, height) = image_dimensions(&bytes)?;
    Some(vision_tile_tokens(width, height))
}

/// 按 OpenAI high-detail 分块公式估算视觉 token。
///
/// 参数:
/// - `width`: 像素宽
/// - `height`: 像素高
///
/// 返回:
/// - 85 + 170 × 横向块数 × 纵向块数
fn vision_tile_tokens(width: u32, height: u32) -> usize {
    if width == 0 || height == 0 {
        return FALLBACK_IMAGE_TOKENS;
    }
    let tiles_x = width.div_ceil(512) as usize;
    let tiles_y = height.div_ceil(512) as usize;
    85usize.saturating_add(170usize.saturating_mul(tiles_x.saturating_mul(tiles_y)))
}

/// 解码 base64 前缀，只取读尺寸所需的字节。
///
/// 参数:
/// - `encoded`: base64 文本
/// - `needed`: 需要的原始字节数
///
/// 返回:
/// - 解码出的前缀字节
fn decode_base64_prefix(encoded: &str, needed: usize) -> Option<Vec<u8>> {
    let take = needed.saturating_mul(2).max(48);
    let slice = encoded.get(..take.min(encoded.len()))?;
    base64::engine::general_purpose::STANDARD.decode(slice).ok()
}

/// 从常见位图头读取宽高。
///
/// 参数:
/// - `bytes`: 图片前若干字节
///
/// 返回:
/// - PNG / JPEG 的宽高
fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    png_dimensions(bytes).or_else(|| jpeg_dimensions(bytes))
}

/// 读取 PNG IHDR 宽高。
///
/// 参数:
/// - `bytes`: PNG 前缀
///
/// 返回:
/// - 宽高
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || bytes.get(..8)? != b"\x89PNG\r\n\x1a\n" {
        return None;
    }
    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    Some((width, height))
}

/// 读取 JPEG SOF 宽高。
///
/// 参数:
/// - `bytes`: JPEG 前缀
///
/// 返回:
/// - 宽高
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut index = 2usize;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xFF {
            return None;
        }
        let marker = bytes[index + 1];
        if marker == 0xD8 || marker == 0x01 {
            index += 2;
            continue;
        }
        if marker == 0xD9 {
            return None;
        }
        let length = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
        if length < 2 || index + 2 + length > bytes.len() {
            return None;
        }
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            let height = u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[index + 7], bytes[index + 8]]) as u32;
            return Some((width, height));
        }
        index += 2 + length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::Usage;

    /// 1×1 PNG 的标准 data URL，用于核对尺寸解析。
    const TINY_PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    /// 验证图片 data URL 不会按 base64 文本计入 token。
    #[test]
    fn image_data_url_is_not_tokenized_as_text() {
        let bulky = format!("data:image/png;base64,{}", "A".repeat(200_000));
        let messages = vec![ChatMessage::user_with_images("hi", [bulky.clone()])];
        let estimated = estimate_chat_messages_tokens(&messages);
        let serialized = token_estimate::estimate_tokens(&bulky);
        assert!(estimated < 10_000, "图片应按视觉口径估算，实际={estimated}");
        assert!(
            estimated < serialized,
            "图片友好估算应低于把 base64 当文本分词"
        );
    }

    /// 验证能从真实 PNG 头读出 1×1 并套用分块公式。
    #[test]
    fn tiny_png_uses_one_vision_tile() {
        assert_eq!(estimate_image_url_tokens(TINY_PNG), 255);
    }

    /// 验证 API 用量存在时占用等于上次 prompt+completion+新用户消息。
    #[test]
    fn occupancy_adds_unsent_user_to_api_usage() {
        let history = vec![
            ChatMessage::plain("user", "old"),
            ChatMessage::plain("assistant", "reply"),
            ChatMessage::plain("user", "new question"),
        ];
        let usage = Usage {
            prompt_tokens: 1_000,
            completion_tokens: 40,
            total_tokens: 1_040,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        };
        let occupancy = occupancy_tokens(&history, Some(&usage));
        let incoming = estimate_message_tokens(&history[2]);
        assert_eq!(occupancy, 1_040 + incoming);
    }

    /// 验证没有 API 用量时回退到全量预估。
    #[test]
    fn occupancy_falls_back_to_full_estimate() {
        let messages = vec![ChatMessage::plain("user", "hello")];
        assert_eq!(
            occupancy_tokens(&messages, None),
            estimate_chat_messages_tokens(&messages)
        );
    }
}

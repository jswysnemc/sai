use crate::i18n::text as t;
use base64::Engine;
use serde_json::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum WeixinInboundMediaKind {
    Image,
    Voice,
    Video,
    File,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct WeixinInboundMedia {
    pub(crate) kind: WeixinInboundMediaKind,
    pub(crate) source: String,
    pub(crate) name: Option<String>,
    pub(crate) download: Option<WeixinInboundMediaDownload>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct WeixinInboundMediaDownload {
    pub(crate) full_url: Option<String>,
    pub(crate) encrypt_query_param: Option<String>,
    pub(crate) aes_key: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct WeixinMessageEvent {
    pub(crate) from_user_id: String,
    pub(crate) context_token: Option<String>,
    pub(crate) prompt: String,
    pub(crate) media: Vec<WeixinInboundMedia>,
}

/// 解析微信 getUpdates 返回中的用户消息。
///
/// 参数:
/// - `message`: 微信消息 JSON
///
/// 返回:
/// - 可交给 Agent 处理的消息事件
pub(crate) fn parse_weixin_message(message: &Value) -> Option<WeixinMessageEvent> {
    if message.get("message_type").and_then(Value::as_i64) != Some(1) {
        return None;
    }
    let from_user_id = message
        .get("from_user_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let context_token = message
        .get("context_token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let items = message
        .get("item_list")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let text = body_from_item_list(items);
    let media = media_from_item_list(items);
    let prompt = build_prompt(&text, &media);
    if prompt.trim().is_empty() {
        return None;
    }
    Some(WeixinMessageEvent {
        from_user_id,
        context_token,
        prompt,
        media,
    })
}

/// 从消息项列表提取文本内容。
///
/// 参数:
/// - `items`: 微信消息项列表
///
/// 返回:
/// - 文本内容
fn body_from_item_list(items: &[Value]) -> String {
    for item in items {
        if item.get("type").and_then(Value::as_i64) == Some(1) {
            if let Some(text) = item
                .get("text_item")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
            {
                return text.trim().to_string();
            }
        }
        if item.get("type").and_then(Value::as_i64) == Some(3) {
            if let Some(text) = item
                .get("voice_item")
                .and_then(|value| value.get("text"))
                .and_then(Value::as_str)
            {
                return text.trim().to_string();
            }
        }
    }
    String::new()
}

/// 从消息项列表提取媒体内容。
///
/// 参数:
/// - `items`: 微信消息项列表
///
/// 返回:
/// - 媒体列表
fn media_from_item_list(items: &[Value]) -> Vec<WeixinInboundMedia> {
    items.iter().filter_map(media_from_item).collect()
}

/// 从单个消息项提取媒体内容。
///
/// 参数:
/// - `item`: 微信消息项
///
/// 返回:
/// - 媒体内容
fn media_from_item(item: &Value) -> Option<WeixinInboundMedia> {
    match item.get("type").and_then(Value::as_i64)? {
        2 => {
            let image = item.get("image_item")?;
            let download =
                media_download(image, first_string(image, &["url"]), image_aes_key(image));
            let source = download
                .as_ref()
                .and_then(WeixinInboundMediaDownload::source)
                .or_else(|| cdn_media_source(image))?;
            Some(WeixinInboundMedia {
                kind: WeixinInboundMediaKind::Image,
                source,
                name: None,
                download,
            })
        }
        3 => {
            let voice = item.get("voice_item")?;
            let download = media_download(voice, None, None);
            let source = download
                .as_ref()
                .and_then(WeixinInboundMediaDownload::source)
                .or_else(|| cdn_media_source(voice))?;
            Some(WeixinInboundMedia {
                kind: WeixinInboundMediaKind::Voice,
                source,
                name: None,
                download,
            })
        }
        4 => {
            let file = item.get("file_item")?;
            let download = media_download(file, None, None);
            let source = download
                .as_ref()
                .and_then(WeixinInboundMediaDownload::source)
                .or_else(|| cdn_media_source(file))?;
            Some(WeixinInboundMedia {
                kind: WeixinInboundMediaKind::File,
                source,
                name: first_string(file, &["file_name"]),
                download,
            })
        }
        5 => {
            let video = item.get("video_item")?;
            let download = media_download(video, None, None);
            let source = download
                .as_ref()
                .and_then(WeixinInboundMediaDownload::source)
                .or_else(|| cdn_media_source(video))?;
            Some(WeixinInboundMedia {
                kind: WeixinInboundMediaKind::Video,
                source,
                name: None,
                download,
            })
        }
        _ => None,
    }
}

/// 从 CDN 媒体字段提取可展示来源。
///
/// 参数:
/// - `value`: 媒体 JSON
///
/// 返回:
/// - 媒体来源
fn cdn_media_source(value: &Value) -> Option<String> {
    value
        .get("media")
        .and_then(|media| first_string(media, &["full_url", "encrypt_query_param"]))
}

/// 从媒体字段提取下载信息。
///
/// 参数:
/// - `value`: 媒体 JSON
/// - `direct_url`: 媒体项直接 URL
/// - `aes_key`: 已规范化的 AES key
///
/// 返回:
/// - 下载信息
fn media_download(
    value: &Value,
    direct_url: Option<String>,
    aes_key: Option<String>,
) -> Option<WeixinInboundMediaDownload> {
    let media = value.get("media");
    let full_url =
        direct_url.or_else(|| media.and_then(|media| first_string(media, &["full_url"])));
    let encrypt_query_param = media.and_then(|media| first_string(media, &["encrypt_query_param"]));
    let aes_key = aes_key.or_else(|| media.and_then(|media| first_string(media, &["aes_key"])));
    if full_url.is_none() && encrypt_query_param.is_none() {
        return None;
    }
    Some(WeixinInboundMediaDownload {
        full_url,
        encrypt_query_param,
        aes_key,
    })
}

/// 提取图片 AES key。
///
/// 参数:
/// - `image`: image_item JSON
///
/// 返回:
/// - base64 编码的 AES key
fn image_aes_key(image: &Value) -> Option<String> {
    let hex_key = first_string(image, &["aeskey"])?;
    let bytes = hex::decode(hex_key).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

impl WeixinInboundMediaDownload {
    /// 返回可展示来源。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 完整 URL 或加密查询参数
    fn source(&self) -> Option<String> {
        self.full_url
            .clone()
            .or_else(|| self.encrypt_query_param.clone())
    }
}

/// 读取 JSON 对象中的第一个非空字符串字段。
///
/// 参数:
/// - `value`: JSON 对象
/// - `keys`: 候选字段名
///
/// 返回:
/// - 第一个非空字符串
fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// 组装交给 Agent 的提示词。
///
/// 参数:
/// - `text`: 文本内容
/// - `media`: 媒体列表
///
/// 返回:
/// - Agent 输入文本
fn build_prompt(text: &str, media: &[WeixinInboundMedia]) -> String {
    let mut parts = Vec::new();
    if !text.trim().is_empty() {
        parts.push(text.trim().to_string());
    }
    for item in media {
        let label = match item.kind {
            WeixinInboundMediaKind::Image => t("image", "图片"),
            WeixinInboundMediaKind::Voice => t("voice message", "语音"),
            WeixinInboundMediaKind::Video => t("video", "视频"),
            WeixinInboundMediaKind::File => t("file", "文件"),
        };
        let name = item.name.as_deref().unwrap_or(t("Unnamed", "未命名"));
        parts.push(format!(
            "{} {label}: {name}",
            t("The user sent", "用户发送了")
        ));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_text_message() {
        let event = parse_weixin_message(&json!({
            "message_type": 1,
            "from_user_id": "user1",
            "context_token": "ctx",
            "item_list": [
                { "type": 1, "text_item": { "text": "你好" } }
            ]
        }))
        .unwrap();

        assert_eq!(event.from_user_id, "user1");
        assert_eq!(event.context_token.as_deref(), Some("ctx"));
        assert_eq!(event.prompt, "你好");
    }

    #[test]
    fn parses_image_message_with_url() {
        let event = parse_weixin_message(&json!({
            "message_type": 1,
            "from_user_id": "user1",
            "item_list": [
                { "type": 2, "image_item": { "url": "https://example.test/a.png" } }
            ]
        }))
        .unwrap();

        assert_eq!(event.media[0].kind, WeixinInboundMediaKind::Image);
        assert_eq!(
            event.media[0]
                .download
                .as_ref()
                .and_then(|download| download.full_url.as_deref()),
            Some("https://example.test/a.png")
        );
    }

    #[test]
    fn parses_file_message_download_fields() {
        let event = parse_weixin_message(&json!({
            "message_type": 1,
            "from_user_id": "user1",
            "item_list": [
                {
                    "type": 4,
                    "file_item": {
                        "file_name": "doc.pdf",
                        "media": {
                            "encrypt_query_param": "param",
                            "aes_key": "YWJjZGVmZ2hpamtsbW5vcA=="
                        }
                    }
                }
            ]
        }))
        .unwrap();

        let media = &event.media[0];
        assert_eq!(media.name.as_deref(), Some("doc.pdf"));
        assert_eq!(
            media
                .download
                .as_ref()
                .and_then(|download| download.encrypt_query_param.as_deref()),
            Some("param")
        );
    }
}

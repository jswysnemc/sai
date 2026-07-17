use super::onebot::OneBotTargetKind;
use anyhow::{bail, Result};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OneBotInboundMediaKind {
    Image,
    File,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OneBotInboundMedia {
    pub(crate) kind: OneBotInboundMediaKind,
    pub(crate) source: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OneBotMessageEvent {
    pub(crate) target_kind: OneBotTargetKind,
    pub(crate) target_id: i64,
    pub(crate) prompt: String,
    pub(crate) media: Vec<OneBotInboundMedia>,
}

/// 解析 OneBot 消息事件。
///
/// 参数:
/// - `payload`: OneBot 上报事件 JSON
///
/// 返回:
/// - 可交给 Agent 处理的消息事件
pub(crate) fn parse_message_event(payload: &Value) -> Result<Option<OneBotMessageEvent>> {
    if payload.get("post_type").and_then(Value::as_str) != Some("message") {
        return Ok(None);
    }
    let message_type = payload
        .get("message_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (target_kind, target_id) = match message_type {
        "private" => {
            let user_id = payload
                .get("user_id")
                .and_then(Value::as_i64)
                .ok_or_else(|| anyhow::anyhow!("missing OneBot private user_id"))?;
            (OneBotTargetKind::Private, user_id)
        }
        "group" => {
            let group_id = payload
                .get("group_id")
                .and_then(Value::as_i64)
                .ok_or_else(|| anyhow::anyhow!("missing OneBot group_id"))?;
            (OneBotTargetKind::Group, group_id)
        }
        value => bail!("unsupported OneBot message_type: {value}"),
    };
    let (text, media) = parse_message_payload(payload.get("message"))?;
    let prompt = build_prompt(&text, &media);
    if prompt.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(OneBotMessageEvent {
        target_kind,
        target_id,
        prompt,
        media,
    }))
}

/// 解析 OneBot message 字段。
///
/// 参数:
/// - `message`: OneBot message 字段
///
/// 返回:
/// - 文本内容和媒体列表
fn parse_message_payload(message: Option<&Value>) -> Result<(String, Vec<OneBotInboundMedia>)> {
    match message {
        Some(Value::String(text)) => Ok(parse_cq_string_message(text)),
        Some(Value::Array(segments)) => parse_message_segments(segments),
        Some(_) => bail!("unsupported OneBot message payload"),
        None => Ok((String::new(), Vec::new())),
    }
}

/// 解析 OneBot 消息段数组。
///
/// 参数:
/// - `segments`: OneBot 消息段数组
///
/// 返回:
/// - 文本内容和媒体列表
fn parse_message_segments(segments: &[Value]) -> Result<(String, Vec<OneBotInboundMedia>)> {
    let mut text_parts = Vec::new();
    let mut media = Vec::new();
    for segment in segments {
        let segment_type = segment
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let data = segment.get("data").unwrap_or(&Value::Null);
        match segment_type {
            "text" => {
                if let Some(text) = data.get("text").and_then(Value::as_str) {
                    let text = text.trim();
                    if !text.is_empty() {
                        text_parts.push(text.to_string());
                    }
                }
            }
            "image" => {
                if let Some(source) = first_string(data, &["url", "file", "path"]) {
                    media.push(OneBotInboundMedia {
                        kind: OneBotInboundMediaKind::Image,
                        source,
                        name: first_string(data, &["name", "filename"]),
                    });
                }
            }
            "file" => {
                if let Some(source) = first_string(data, &["url", "file", "path"]) {
                    media.push(OneBotInboundMedia {
                        kind: OneBotInboundMediaKind::File,
                        source,
                        name: first_string(data, &["name", "filename"]),
                    });
                }
            }
            _ => {}
        }
    }
    Ok((text_parts.join("\n"), media))
}

/// 解析 CQ 字符串消息。
///
/// 参数:
/// - `message`: OneBot CQ 字符串
///
/// 返回:
/// - 文本内容和媒体列表
fn parse_cq_string_message(message: &str) -> (String, Vec<OneBotInboundMedia>) {
    let mut rest = message;
    let mut text_parts = Vec::new();
    let mut media = Vec::new();
    while let Some(start) = rest.find("[CQ:") {
        let text = rest[..start].trim();
        if !text.is_empty() {
            text_parts.push(text.to_string());
        }
        let after_start = &rest[start..];
        let Some(end) = after_start.find(']') else {
            break;
        };
        if let Some(item) = parse_cq_media_segment(&after_start[..=end]) {
            media.push(item);
        }
        rest = &after_start[end + 1..];
    }
    let trailing = rest.trim();
    if !trailing.is_empty() {
        text_parts.push(trailing.to_string());
    }
    if text_parts.is_empty() && media.is_empty() {
        text_parts.push(message.trim().to_string());
    }
    (text_parts.join("\n"), media)
}

/// 解析单个 CQ 媒体段。
///
/// 参数:
/// - `segment`: CQ 段文本
///
/// 返回:
/// - 入站媒体
fn parse_cq_media_segment(segment: &str) -> Option<OneBotInboundMedia> {
    let content = segment.strip_prefix("[CQ:")?.strip_suffix(']')?;
    let mut fields = content.split(',');
    let segment_type = fields.next()?.trim();
    let kind = match segment_type {
        "image" => OneBotInboundMediaKind::Image,
        "file" => OneBotInboundMediaKind::File,
        _ => return None,
    };
    let mut source = None;
    let mut name = None;
    for field in fields {
        let Some((key, value)) = field.split_once('=') else {
            continue;
        };
        let value = cq_unescape(value.trim());
        match key.trim() {
            "url" | "path" => {
                if source.is_none() && !value.trim().is_empty() {
                    source = Some(value);
                }
            }
            "file" => {
                if !value.trim().is_empty() {
                    if source.is_none() {
                        source = Some(value.clone());
                    }
                    if name.is_none() {
                        name = Some(value);
                    }
                }
            }
            "name" | "filename" => {
                if name.is_none() && !value.trim().is_empty() {
                    name = Some(value);
                }
            }
            _ => {}
        }
    }
    Some(OneBotInboundMedia {
        kind,
        source: source?,
        name,
    })
}

/// 反转义 CQ 参数中的基础实体。
///
/// 参数:
/// - `value`: CQ 参数值
///
/// 返回:
/// - 反转义后的参数值
fn cq_unescape(value: &str) -> String {
    value
        .replace("&#91;", "[")
        .replace("&#93;", "]")
        .replace("&amp;", "&")
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
/// - `text`: 用户文本
/// - `media`: 媒体列表
///
/// 返回:
/// - Agent 输入文本
fn build_prompt(text: &str, media: &[OneBotInboundMedia]) -> String {
    let mut parts = Vec::new();
    if !text.trim().is_empty() {
        parts.push(text.trim().to_string());
    }
    for item in media {
        let label = match item.kind {
            OneBotInboundMediaKind::Image => "图片",
            OneBotInboundMediaKind::File => "文件",
        };
        let name = item
            .name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("未命名");
        parts.push(format!("{label}: {name}\n来源: {}", item.source));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_group_text_and_image_message() {
        let event = parse_message_event(&json!({
            "post_type": "message",
            "message_type": "group",
            "group_id": 10001,
            "message": [
                { "type": "text", "data": { "text": "  分析这张图  " } },
                { "type": "image", "data": { "url": "https://example.test/a.png", "file": "a.png" } }
            ]
        }))
        .unwrap()
        .unwrap();

        assert_eq!(event.target_kind, OneBotTargetKind::Group);
        assert_eq!(event.target_id, 10001);
        assert!(event.prompt.contains("分析这张图"));
        assert_eq!(event.media.len(), 1);
        assert_eq!(event.media[0].kind, OneBotInboundMediaKind::Image);
    }

    #[test]
    fn ignores_non_message_event() {
        let event = parse_message_event(&json!({
            "post_type": "notice"
        }))
        .unwrap();

        assert!(event.is_none());
    }

    #[test]
    fn parses_cq_string_image_and_file_message() {
        let event = parse_message_event(&json!({
            "post_type": "message",
            "message_type": "private",
            "user_id": 20002,
            "message": "看附件 [CQ:image,file=a.png,url=https://example.test/a.png] [CQ:file,file=report.pdf,url=https://example.test/report.pdf]"
        }))
        .unwrap()
        .unwrap();

        assert_eq!(event.target_kind, OneBotTargetKind::Private);
        assert_eq!(event.target_id, 20002);
        assert!(event.prompt.contains("看附件"));
        assert_eq!(event.media.len(), 2);
        assert_eq!(event.media[0].kind, OneBotInboundMediaKind::Image);
        assert_eq!(event.media[1].kind, OneBotInboundMediaKind::File);
    }
}

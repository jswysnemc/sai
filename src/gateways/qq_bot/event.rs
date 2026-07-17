use crate::gateways::qq_official::QqTargetKind;
use anyhow::{bail, Result};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QqBotInboundMediaKind {
    Image,
    Voice,
    Video,
    File,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct QqBotInboundMedia {
    pub(crate) kind: QqBotInboundMediaKind,
    pub(crate) source: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct QqBotMessageEvent {
    pub(crate) event_type: String,
    pub(crate) target_kind: QqTargetKind,
    pub(crate) target_id: String,
    pub(crate) msg_id: String,
    pub(crate) prompt: String,
    pub(crate) media: Vec<QqBotInboundMedia>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct QqBotValidationEvent {
    pub(crate) plain_token: String,
    pub(crate) event_ts: String,
}

/// 解析 QQ Webhook 回调地址验证事件。
///
/// 参数:
/// - `payload`: QQ Webhook Payload
///
/// 返回:
/// - 回调验证事件
pub(crate) fn parse_validation_event(payload: &Value) -> Result<Option<QqBotValidationEvent>> {
    if payload.get("op").and_then(Value::as_i64) != Some(13) {
        return Ok(None);
    }
    let data = payload.get("d").unwrap_or(&Value::Null);
    let plain_token = data
        .get("plain_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let event_ts = data
        .get("event_ts")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if plain_token.is_empty() || event_ts.is_empty() {
        bail!("invalid QQ webhook validation payload");
    }
    Ok(Some(QqBotValidationEvent {
        plain_token,
        event_ts,
    }))
}

/// 解析 QQ 官方机器人消息事件。
///
/// 参数:
/// - `payload`: QQ Webhook Payload
///
/// 返回:
/// - 可交给 Agent 处理的消息事件
pub(crate) fn parse_message_event(payload: &Value) -> Result<Option<QqBotMessageEvent>> {
    if payload.get("op").and_then(Value::as_i64) != Some(0) {
        return Ok(None);
    }
    let event_type = payload
        .get("t")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let data = payload.get("d").unwrap_or(&Value::Null);
    let target_kind = match event_type.as_str() {
        "C2C_MESSAGE_CREATE" => QqTargetKind::User,
        "GROUP_AT_MESSAGE_CREATE" | "GROUP_MESSAGE_CREATE" => QqTargetKind::Group,
        _ => return Ok(None),
    };
    let target_id = match target_kind {
        QqTargetKind::User => data
            .get("author")
            .and_then(|author| author.get("user_openid").or_else(|| author.get("id")))
            .and_then(Value::as_str),
        QqTargetKind::Group => data.get("group_openid").and_then(Value::as_str),
    }
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .ok_or_else(|| anyhow::anyhow!("missing QQ target id"))?
    .to_string();
    let msg_id = data
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("missing QQ message id"))?
        .to_string();
    let text = data
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let media = parse_attachments(data.get("attachments"));
    let prompt = build_prompt(&text, &media);
    if prompt.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(QqBotMessageEvent {
        event_type,
        target_kind,
        target_id,
        msg_id,
        prompt,
        media,
    }))
}

/// 解析 QQ 附件列表。
///
/// 参数:
/// - `attachments`: 附件 JSON 字段
///
/// 返回:
/// - 入站媒体列表
fn parse_attachments(attachments: Option<&Value>) -> Vec<QqBotInboundMedia> {
    let Some(Value::Array(items)) = attachments else {
        return Vec::new();
    };
    items.iter().filter_map(parse_attachment).collect()
}

/// 解析单个 QQ 附件。
///
/// 参数:
/// - `item`: 附件 JSON
///
/// 返回:
/// - 入站媒体
fn parse_attachment(item: &Value) -> Option<QqBotInboundMedia> {
    let source = first_string(item, &["url", "file_url", "download_url"])?;
    let content_type = first_string(item, &["content_type", "contentType"]).unwrap_or_default();
    let filename = first_string(item, &["filename", "file_name", "name"]);
    let kind = if content_type.starts_with("image/") {
        QqBotInboundMediaKind::Image
    } else if content_type.starts_with("audio/") {
        QqBotInboundMediaKind::Voice
    } else if content_type.starts_with("video/") {
        QqBotInboundMediaKind::Video
    } else {
        QqBotInboundMediaKind::File
    };
    Some(QqBotInboundMedia {
        kind,
        source,
        name: filename,
    })
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
fn build_prompt(text: &str, media: &[QqBotInboundMedia]) -> String {
    let mut parts = Vec::new();
    if !text.trim().is_empty() {
        parts.push(text.trim().to_string());
    }
    for item in media {
        let label = match item.kind {
            QqBotInboundMediaKind::Image => "图片",
            QqBotInboundMediaKind::Voice => "语音",
            QqBotInboundMediaKind::Video => "视频",
            QqBotInboundMediaKind::File => "文件",
        };
        let name = item.name.as_deref().unwrap_or("未命名");
        parts.push(format!("{label}: {name}\n来源: {}", item.source));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_validation_event() {
        let event = parse_validation_event(&json!({
            "op": 13,
            "d": {
                "plain_token": "token",
                "event_ts": "1725442341"
            }
        }))
        .unwrap()
        .unwrap();

        assert_eq!(event.plain_token, "token");
        assert_eq!(event.event_ts, "1725442341");
    }

    #[test]
    fn parses_c2c_message_event() {
        let event = parse_message_event(&json!({
            "op": 0,
            "t": "C2C_MESSAGE_CREATE",
            "d": {
                "id": "msg-1",
                "content": "你好",
                "author": { "user_openid": "user-openid" }
            }
        }))
        .unwrap()
        .unwrap();

        assert_eq!(event.target_kind, QqTargetKind::User);
        assert_eq!(event.target_id, "user-openid");
        assert_eq!(event.msg_id, "msg-1");
        assert_eq!(event.prompt, "你好");
    }

    #[test]
    fn parses_group_message_with_image_attachment() {
        let event = parse_message_event(&json!({
            "op": 0,
            "t": "GROUP_AT_MESSAGE_CREATE",
            "d": {
                "id": "msg-2",
                "content": "看图",
                "group_openid": "group-openid",
                "attachments": [
                    {
                        "url": "https://example.test/a.png",
                        "content_type": "image/png",
                        "filename": "a.png"
                    }
                ]
            }
        }))
        .unwrap()
        .unwrap();

        assert_eq!(event.target_kind, QqTargetKind::Group);
        assert_eq!(event.target_id, "group-openid");
        assert_eq!(event.media[0].kind, QqBotInboundMediaKind::Image);
    }
}

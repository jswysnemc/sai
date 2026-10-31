use super::message::{MediaKind, OutboundMedia, OutboundMessage};
use super::qq_official::QqOfficialClient;
use super::weixin_bot::client::WeixinBotClient;
use super::weixin_bot::media::{send_local_media, WeixinOutboundMediaKind};
use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::path::PathBuf;

/// 发送 QQ 渠道消息。
///
/// 参数:
/// - `client`: QQ 官方客户端
/// - `msg_id`: 可选消息 ID
/// - `kind`: 消息类型
/// - `text`: 可选文本
/// - `path`: 可选本机文件路径
///
/// 返回:
/// - JSON 格式发送结果
pub(super) async fn send_qq_message(
    client: QqOfficialClient,
    msg_id: Option<String>,
    kind: &str,
    text: Option<String>,
    path: Option<PathBuf>,
) -> Result<String> {
    let message = match kind {
        "text" => OutboundMessage {
            text: Some(required_text(text)?),
            media: Vec::new(),
        },
        "image" => OutboundMessage {
            text,
            media: vec![OutboundMedia {
                kind: MediaKind::Image,
                path: required_path(path)?,
            }],
        },
        "file" => OutboundMessage {
            text,
            media: vec![OutboundMedia {
                kind: MediaKind::File,
                path: required_path(path)?,
            }],
        },
        "video" => bail!(t(
            "QQ channel does not support video via send_channel_message",
            "QQ 渠道不支持通过 send_channel_message 发送视频"
        )),
        value => bail!(
            "{}: {value}",
            t("unsupported message kind", "不支持的消息类型")
        ),
    };
    let responses = client.send(&message, msg_id.as_deref()).await?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "channel": "qq",
        "kind": kind,
        "responses": responses,
    }))?)
}

/// 发送微信渠道消息。
///
/// 参数:
/// - `client`: 微信客户端
/// - `to_user_id`: 接收方用户 ID
/// - `context_token`: 可选上下文 token
/// - `kind`: 消息类型
/// - `text`: 可选文本
/// - `path`: 可选本机文件路径
///
/// 返回:
/// - JSON 格式发送结果
pub(super) async fn send_weixin_message(
    client: WeixinBotClient,
    to_user_id: String,
    context_token: Option<String>,
    kind: &str,
    text: Option<String>,
    path: Option<PathBuf>,
) -> Result<String> {
    match kind {
        "text" => {
            client
                .send_text(&to_user_id, &required_text(text)?, context_token.as_deref())
                .await?;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "channel": "weixin",
                "kind": "text",
            }))?)
        }
        "image" | "file" | "video" => {
            let kind = match kind {
                "image" => WeixinOutboundMediaKind::Image,
                "file" => WeixinOutboundMediaKind::File,
                "video" => WeixinOutboundMediaKind::Video,
                _ => unreachable!(),
            };
            let message_id = send_local_media(
                &client,
                &to_user_id,
                context_token.as_deref(),
                &required_path(path)?,
                text.as_deref(),
                kind,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "channel": "weixin",
                "kind": media_kind_name(kind),
                "message_id": message_id,
            }))?)
        }
        value => bail!(
            "{}: {value}",
            t("unsupported message kind", "不支持的消息类型")
        ),
    }
}

/// 读取必填文本。
///
/// 参数:
/// - `text`: 可选文本
///
/// 返回:
/// - 文本
pub(super) fn required_text(text: Option<String>) -> Result<String> {
    text.filter(|value| !value.trim().is_empty()).context(t(
        "text is required for text messages",
        "文本消息必须提供 text",
    ))
}

/// 读取必填路径。
///
/// 参数:
/// - `path`: 可选路径
///
/// 返回:
/// - 路径
pub(super) fn required_path(path: Option<PathBuf>) -> Result<PathBuf> {
    path.context(t(
        "path is required for media messages",
        "媒体消息必须提供 path",
    ))
}

/// 返回微信媒体类型名称。
///
/// 参数:
/// - `kind`: 微信媒体类型
///
/// 返回:
/// - 媒体类型文本
fn media_kind_name(kind: WeixinOutboundMediaKind) -> &'static str {
    match kind {
        WeixinOutboundMediaKind::Image => "image",
        WeixinOutboundMediaKind::Video => "video",
        WeixinOutboundMediaKind::File => "file",
    }
}

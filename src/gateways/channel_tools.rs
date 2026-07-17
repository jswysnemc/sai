use super::channel_context::{load_latest_channel_context, ChannelContext};
use super::message::{MediaKind, OutboundMedia, OutboundMessage};
use super::qq_bot::auth::QqBotAuthenticator;
use super::qq_bot::config::{resolve_qq_credentials, QqBotCredentialOverrides};
use super::qq_official::{QqOfficialClient, QqTargetKind};
use super::weixin_bot::client::WeixinBotClient;
use super::weixin_bot::login::{default_base_url as default_weixin_base_url, load_weixin_account};
use super::weixin_bot::media::{send_local_media, WeixinOutboundMediaKind};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) enum ActiveChannelTarget {
    Qq {
        client: QqOfficialClient,
        msg_id: Option<String>,
    },
    Weixin {
        client: WeixinBotClient,
        to_user_id: String,
        context_token: Option<String>,
    },
}

/// 注册统一渠道发送工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `current`: 当前入站消息对应渠道目标
///
/// 返回:
/// - 无
pub(crate) fn register_channel_message_tool(
    registry: &mut ToolRegistry,
    paths: SaiPaths,
    config: AppConfig,
    current: ActiveChannelTarget,
) {
    registry.register(
        ToolSpec::new(
            "send_channel_message",
            "Send text or a local media file to a channel. Use channel=current for the current chat, or channel=qq/weixin to send to that channel's most recent chat.",
            json!({
                "type": "object",
                "properties": {
                    "channel": {
                        "type": "string",
                        "description": "Target channel: current, qq, or weixin.",
                        "enum": ["current", "qq", "weixin"]
                    },
                    "kind": {
                        "type": "string",
                        "description": "Message kind: text, image, file, or video.",
                        "enum": ["text", "image", "file", "video"]
                    },
                    "text": {
                        "type": "string",
                        "description": "Text message content, or optional text sent before media."
                    },
                    "path": {
                        "type": "string",
                        "description": "Local file path for image, file, or video."
                    },
                    "caption": {
                        "type": "string",
                        "description": "Optional media caption. If text is also provided, text takes precedence."
                    }
                },
                "required": ["kind"],
                "additionalProperties": false
            }),
            move |args| {
                let paths = paths.clone();
                let config = config.clone();
                let current = current.clone();
                async move { send_channel_message(args, paths, config, current).await }
            },
        )
        .writes(),
    );
}

/// 根据持久化渠道上下文重建当前发送目标。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `context`: 会话绑定的渠道上下文
///
/// 返回:
/// - 可供渠道发送工具使用的目标
pub(crate) async fn resolve_channel_target(
    paths: &SaiPaths,
    config: &AppConfig,
    context: &ChannelContext,
) -> Result<ActiveChannelTarget> {
    match context {
        ChannelContext::Qq {
            target_kind,
            target_id,
            msg_id,
            ..
        } => resolve_qq_context_target(config, target_kind, target_id, msg_id.clone()).await,
        ChannelContext::Weixin {
            to_user_id,
            context_token,
            ..
        } => {
            resolve_weixin_context_target(paths, config, to_user_id.clone(), context_token.clone())
        }
    }
}

/// 执行统一渠道发送工具。
///
/// 参数:
/// - `args`: 工具参数
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `current`: 当前入站消息对应渠道目标
///
/// 返回:
/// - JSON 格式发送结果
async fn send_channel_message(
    args: Value,
    paths: SaiPaths,
    config: AppConfig,
    current: ActiveChannelTarget,
) -> Result<String> {
    let channel = optional_string(&args, "channel").unwrap_or_else(|| "current".to_string());
    let kind = required_string(&args, "kind")?;
    let text = optional_string(&args, "text").or_else(|| optional_string(&args, "caption"));
    let path = optional_string(&args, "path").map(PathBuf::from);
    let target = resolve_target(&paths, &config, current, &channel).await?;
    match target {
        ActiveChannelTarget::Qq { client, msg_id } => {
            send_qq_message(client, msg_id, &kind, text, path).await
        }
        ActiveChannelTarget::Weixin {
            client,
            to_user_id,
            context_token,
        } => send_weixin_message(client, to_user_id, context_token, &kind, text, path).await,
    }
}

/// 解析发送目标。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `current`: 当前渠道目标
/// - `channel`: 目标渠道
///
/// 返回:
/// - 实际发送目标
async fn resolve_target(
    paths: &SaiPaths,
    config: &AppConfig,
    current: ActiveChannelTarget,
    channel: &str,
) -> Result<ActiveChannelTarget> {
    match channel.trim().to_ascii_lowercase().as_str() {
        "" | "current" => Ok(current),
        "qq" => resolve_qq_target(paths, config).await,
        "weixin" | "wechat" => resolve_weixin_target(paths, config),
        value => bail!("unsupported channel: {value}"),
    }
}

/// 解析最近 QQ 发送目标。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - QQ 发送目标
async fn resolve_qq_target(paths: &SaiPaths, config: &AppConfig) -> Result<ActiveChannelTarget> {
    let Some(ChannelContext::Qq {
        target_kind,
        target_id,
        msg_id,
        ..
    }) = load_latest_channel_context(paths, "qq")?
    else {
        bail!("no recent QQ channel target");
    };
    resolve_qq_context_target(config, &target_kind, &target_id, msg_id).await
}

/// 根据明确 QQ 上下文重建发送目标。
///
/// 参数:
/// - `config`: 应用配置
/// - `target_kind`: QQ 目标类型
/// - `target_id`: QQ 目标标识
/// - `msg_id`: 可选被动回复消息标识
///
/// 返回:
/// - QQ 发送目标
async fn resolve_qq_context_target(
    config: &AppConfig,
    target_kind: &str,
    target_id: &str,
    msg_id: Option<String>,
) -> Result<ActiveChannelTarget> {
    let credentials = resolve_qq_credentials(
        QqBotCredentialOverrides {
            token: None,
            app_id: None,
            client_secret: None,
        },
        &config.gateways.qq,
    )?;
    let mut authenticator = QqBotAuthenticator::new(credentials.app_id, credentials.client_secret);
    let authorization = authenticator.authorization().await?;
    let target_kind = QqTargetKind::parse(target_kind)?;
    let client = QqOfficialClient::new(
        non_empty_config(&config.gateways.qq.base_url)
            .unwrap_or_else(|| "https://api.sgroup.qq.com".to_string()),
        authorization,
        target_kind,
        target_id.to_string(),
    );
    Ok(ActiveChannelTarget::Qq { client, msg_id })
}

/// 解析最近微信发送目标。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - 微信发送目标
fn resolve_weixin_target(paths: &SaiPaths, config: &AppConfig) -> Result<ActiveChannelTarget> {
    let Some(ChannelContext::Weixin {
        to_user_id,
        context_token,
        ..
    }) = load_latest_channel_context(paths, "weixin")?
    else {
        bail!("no recent Weixin channel target");
    };
    resolve_weixin_context_target(paths, config, to_user_id, context_token)
}

/// 根据明确微信上下文重建发送目标。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `to_user_id`: 微信用户标识
/// - `context_token`: 可选上下文 token
///
/// 返回:
/// - 微信发送目标
fn resolve_weixin_context_target(
    paths: &SaiPaths,
    config: &AppConfig,
    to_user_id: String,
    context_token: Option<String>,
) -> Result<ActiveChannelTarget> {
    let weixin = &config.gateways.weixin;
    let base_url_config =
        non_empty_config(&weixin.base_url).unwrap_or_else(|| default_weixin_base_url().to_string());
    let cdn_base_url_config = non_empty_config(&weixin.cdn_base_url)
        .unwrap_or_else(|| super::weixin_bot::client::default_cdn_base_url().to_string());
    let (base_url, cdn_base_url, token) = if let Some(token) = non_empty_config(&weixin.token) {
        (base_url_config, cdn_base_url_config, token)
    } else {
        let account = load_weixin_account(paths, non_empty_config(&weixin.account).as_deref())?;
        (account.base_url, account.cdn_base_url, account.token)
    };
    let client = WeixinBotClient::new(
        base_url,
        cdn_base_url,
        token,
        non_empty_config(&weixin.bot_agent),
        false,
    );
    Ok(ActiveChannelTarget::Weixin {
        client,
        to_user_id,
        context_token,
    })
}

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
async fn send_qq_message(
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
        "video" => bail!("QQ channel does not support video via send_channel_message"),
        value => bail!("unsupported message kind: {value}"),
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
async fn send_weixin_message(
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
        value => bail!("unsupported message kind: {value}"),
    }
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 字段名
///
/// 返回:
/// - 字符串值
fn required_string(args: &Value, key: &str) -> Result<String> {
    optional_string(args, key).with_context(|| format!("{key} is required"))
}

/// 读取可选字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 字段名
///
/// 返回:
/// - 可选字符串值
fn optional_string(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// 读取必填文本。
///
/// 参数:
/// - `text`: 可选文本
///
/// 返回:
/// - 文本
fn required_text(text: Option<String>) -> Result<String> {
    text.filter(|value| !value.trim().is_empty())
        .context("text is required for text messages")
}

/// 读取必填路径。
///
/// 参数:
/// - `path`: 可选路径
///
/// 返回:
/// - 路径
fn required_path(path: Option<PathBuf>) -> Result<PathBuf> {
    path.context("path is required for media messages")
}

/// 读取非空配置字符串。
///
/// 参数:
/// - `value`: 配置值
///
/// 返回:
/// - 非空字符串
fn non_empty_config(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证可选字符串参数会去除首尾空白并忽略空值。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn optional_string_trims_and_ignores_empty_values() {
        let args = json!({
            "channel": " qq ",
            "text": "   ",
            "count": 1
        });

        assert_eq!(optional_string(&args, "channel").as_deref(), Some("qq"));
        assert_eq!(optional_string(&args, "text"), None);
        assert_eq!(optional_string(&args, "count"), None);
    }

    /// 验证必填字符串参数缺失时返回错误。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn required_string_rejects_missing_values() {
        let err = required_string(&json!({}), "kind").unwrap_err();

        assert!(err.to_string().contains("kind is required"));
    }

    /// 验证文本消息必须提供非空文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn required_text_rejects_blank_text() {
        let err = required_text(Some("   ".to_string())).unwrap_err();

        assert!(err.to_string().contains("text is required"));
    }

    /// 验证媒体消息必须提供本机路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn required_path_rejects_missing_path() {
        let err = required_path(None).unwrap_err();

        assert!(err.to_string().contains("path is required"));
    }

    /// 验证非空配置读取会去除空白。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn non_empty_config_trims_values() {
        assert_eq!(
            non_empty_config(" https://example.invalid "),
            Some("https://example.invalid".to_string())
        );
        assert_eq!(non_empty_config("  "), None);
    }
}

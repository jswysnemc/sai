use crate::config::QqGatewayConfig;
use anyhow::{bail, Context, Result};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QqBotTransport {
    Websocket,
    Webhook,
}

impl QqBotTransport {
    /// 解析 QQ 官方机器人传输模式。
    ///
    /// 参数:
    /// - `value`: 传输模式文本
    ///
    /// 返回:
    /// - QQ 官方机器人传输模式
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "websocket" | "ws" => Ok(Self::Websocket),
            "webhook" | "http" => Ok(Self::Webhook),
            _ => bail!("unsupported QQ bot transport: {value}"),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct QqBotCredentials {
    pub(crate) app_id: String,
    pub(crate) client_secret: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct QqBotCredentialOverrides<'a> {
    pub(crate) token: Option<&'a str>,
    pub(crate) app_id: Option<&'a str>,
    pub(crate) client_secret: Option<&'a str>,
}

/// 解析 QQ 官方机器人实际传输模式。
///
/// 参数:
/// - `arg`: 命令行传输模式
/// - `configured`: TUI 保存的传输模式
///
/// 返回:
/// - QQ 官方机器人传输模式
pub(crate) fn resolve_qq_transport(arg: Option<&str>, configured: &str) -> Result<QqBotTransport> {
    if let Some(value) = arg.map(str::trim).filter(|value| !value.is_empty()) {
        return QqBotTransport::parse(value);
    }
    QqBotTransport::parse(configured)
}

/// 解析 QQ 官方机器人认证信息。
///
/// 参数:
/// - `overrides`: 命令行覆盖参数
/// - `configured`: TUI 保存的 QQ 配置
///
/// 返回:
/// - QQ AppID 和 AppSecret
pub(crate) fn resolve_qq_credentials(
    overrides: QqBotCredentialOverrides<'_>,
    configured: &QqGatewayConfig,
) -> Result<QqBotCredentials> {
    let cli_token = parse_optional_qq_token(overrides.token)?;
    let configured_token = parse_optional_qq_token(Some(&configured.token))?;
    let app_id = non_empty_ref(overrides.app_id)
        .or_else(|| cli_token.as_ref().map(|token| token.app_id.clone()))
        .or_else(|| non_empty_string(&configured.app_id))
        .or_else(|| configured_token.as_ref().map(|token| token.app_id.clone()))
        .context("provide --token, --app-id, or configure QQ credentials in TUI")?;
    let client_secret = non_empty_ref(overrides.client_secret)
        .or_else(|| cli_token.map(|token| token.client_secret))
        .or_else(|| non_empty_string(&configured.client_secret))
        .or_else(|| configured_token.map(|token| token.client_secret))
        .context("provide --token, --client-secret, or configure QQ credentials in TUI")?;
    Ok(QqBotCredentials {
        app_id,
        client_secret,
    })
}

/// 解析 QQ Token。
///
/// 参数:
/// - `value`: Token 文本，格式为 AppID:AppSecret
///
/// 返回:
/// - QQ 认证信息
pub(crate) fn parse_qq_token(value: &str) -> Result<QqBotCredentials> {
    let (app_id, client_secret) = value
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("QQ token must use AppID:AppSecret format"))?;
    let app_id = app_id.trim();
    let client_secret = client_secret.trim();
    if app_id.is_empty() || client_secret.is_empty() {
        bail!("QQ token must include both AppID and AppSecret");
    }
    Ok(QqBotCredentials {
        app_id: app_id.to_string(),
        client_secret: client_secret.to_string(),
    })
}

/// 解析可选 QQ Token。
///
/// 参数:
/// - `value`: Token 文本，格式为 AppID:AppSecret
///
/// 返回:
/// - 可选 QQ 认证信息
fn parse_optional_qq_token(value: Option<&str>) -> Result<Option<QqBotCredentials>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    parse_qq_token(value).map(Some)
}

/// 读取非空字符串引用。
///
/// 参数:
/// - `value`: 字符串引用
///
/// 返回:
/// - 非空字符串
fn non_empty_ref(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

/// 读取非空配置字符串。
///
/// 参数:
/// - `value`: 配置字符串
///
/// 返回:
/// - 非空字符串
fn non_empty_string(value: &str) -> Option<String> {
    non_empty_ref(Some(value))
}

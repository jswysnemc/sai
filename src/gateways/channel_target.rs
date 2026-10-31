use super::channel_context::{load_latest_channel_context, ChannelContext};
use super::qq_bot::auth::QqBotAuthenticator;
use super::qq_bot::config::{resolve_qq_credentials, QqBotCredentialOverrides};
use super::qq_official::{QqOfficialClient, QqTargetKind};
use super::weixin_bot::client::WeixinBotClient;
use super::weixin_bot::login::{default_base_url as default_weixin_base_url, load_weixin_account};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};

#[derive(Clone)]
pub(crate) enum ActiveChannelTarget {
    /// 【渠道消息】【普通会话】没有入站绑定时，在实际发送时读取最近微信目标
    Unbound,
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

/// 【渠道消息】【最近目标】根据 paths/config/channel 重建最近目标，返回发送客户端。
/// 无可用上下文时明确失败，不猜测接收人
pub(super) async fn resolve_latest_target(
    paths: &SaiPaths,
    config: &AppConfig,
    channel: &str,
) -> Result<ActiveChannelTarget> {
    let channel = if channel == "wechat" {
        "weixin"
    } else {
        channel
    };
    let context = load_latest_channel_context(paths, channel)?.with_context(|| {
        format!(
            "{}: {channel}. {}",
            t("no recent channel target", "没有最近使用的渠道目标"),
            t(
                "Send a message to the Sai bot in that channel first.",
                "请先在该渠道向 Sai 机器人发送一条消息。"
            )
        )
    })?;
    if context.channel() != channel {
        bail!("channel context does not match requested channel: {channel}");
    }
    resolve_channel_target(paths, config, &context).await
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
        // 飞书暂未接主动推送工具：回复走网关自身的发送路径
        ChannelContext::Feishu { .. } => bail!(t(
            "the Feishu channel does not support proactive messages yet",
            "飞书渠道暂不支持主动推送消息",
        )),
    }
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
    let client = configured_weixin_client(paths, config)?;
    Ok(ActiveChannelTarget::Weixin {
        client,
        to_user_id,
        context_token,
    })
}

/// 【渠道消息】【微信客户端】参数 paths/config 为运行路径与配置，返回无需接收人即可启动的客户端。
pub(crate) fn configured_weixin_client(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<WeixinBotClient> {
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
    Ok(WeixinBotClient::new(
        base_url,
        cdn_base_url,
        token,
        non_empty_config(&weixin.bot_agent),
        false,
    ))
}

/// 读取非空配置字符串。
///
/// 参数:
/// - `value`: 配置值
///
/// 返回:
/// - 非空字符串
pub(super) fn non_empty_config(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

use super::qq_bot::config::{
    resolve_qq_credentials, resolve_qq_transport, QqBotCredentialOverrides, QqBotTransport,
};
use super::qq_bot::webhook_server::{run_qq_bot_webhook_server, QqBotWebhookServerConfig};
use super::qq_bot::websocket::{run_qq_bot_websocket, QqBotWebsocketConfig};
use super::weixin_bot::client::default_cdn_base_url as default_weixin_cdn_base_url;
use super::weixin_bot::login::{default_base_url as default_weixin_base_url, load_weixin_account};
use super::weixin_bot::server::{run_weixin_bot_server, WeixinBotServerConfig};
use crate::config::{AppConfig, QqGatewayConfig, WeixinGatewayConfig};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::net::SocketAddr;
use tokio::task::JoinSet;

enum ConfiguredGateway {
    QqWebsocket(QqBotWebsocketConfig),
    QqWebhook(QqBotWebhookServerConfig),
    Weixin(WeixinBotServerConfig),
}

impl ConfiguredGateway {
    /// 返回渠道名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 渠道名称
    fn name(&self) -> &'static str {
        match self {
            Self::QqWebsocket(_) => "qq-bot-websocket",
            Self::QqWebhook(_) => "qq-bot-webhook",
            Self::Weixin(_) => "weixin-server",
        }
    }
}

/// 启动 TUI 配置中已启用的渠道网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `verbose`: 是否输出详细日志
///
/// 返回:
/// - 网关运行结果
pub(crate) async fn run_configured_gateways(paths: &SaiPaths, verbose: bool) -> Result<()> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    let gateways = configured_gateways(paths, &config, verbose)?;
    let names = gateways
        .iter()
        .map(ConfiguredGateway::name)
        .collect::<Vec<_>>();
    let mut tasks = JoinSet::<Result<()>>::new();
    tasks.spawn(crate::cron::run_scheduler(paths.clone()));
    for gateway in gateways {
        spawn_gateway(&mut tasks, paths.clone(), gateway);
    }
    println!(
        "{}: {}",
        t("Configured gateways started", "已启动配置网关"),
        names.join(", ")
    );
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tasks.abort_all();
            println!("{}", t("Configured gateways stopped", "配置网关已停止"));
            Ok(())
        }
        result = tasks.join_next() => {
            tasks.abort_all();
            match result {
                Some(Ok(Ok(()))) => Ok(()),
                Some(Ok(Err(err))) => Err(err),
                Some(Err(err)) => Err(anyhow::anyhow!(
                    "{}: {err}",
                    t("gateway task failed", "网关任务失败")
                )),
                None => Ok(()),
            }
        }
    }
}

/// 组装已启用渠道网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `verbose`: 是否输出详细日志
///
/// 返回:
/// - 已启用网关列表
fn configured_gateways(
    paths: &SaiPaths,
    config: &AppConfig,
    verbose: bool,
) -> Result<Vec<ConfiguredGateway>> {
    let mut gateways = Vec::new();
    if config.gateways.qq.enabled {
        gateways.push(configured_qq_gateway(&config.gateways.qq, verbose)?);
    }
    if config.gateways.weixin.enabled {
        gateways.push(configured_weixin_gateway(
            paths,
            &config.gateways.weixin,
            verbose,
        )?);
    }
    Ok(gateways)
}

/// 组装 QQ 官方机器人网关配置。
///
/// 参数:
/// - `qq`: QQ 渠道配置
/// - `verbose`: 是否输出详细日志
///
/// 返回:
/// - 已配置 QQ 网关
fn configured_qq_gateway(qq: &QqGatewayConfig, verbose: bool) -> Result<ConfiguredGateway> {
    let credentials = resolve_qq_credentials(
        QqBotCredentialOverrides {
            token: None,
            app_id: None,
            client_secret: None,
        },
        qq,
    )?;
    let base_url =
        non_empty_config(&qq.base_url).unwrap_or_else(|| "https://api.sgroup.qq.com".to_string());
    match resolve_qq_transport(None, &qq.transport)? {
        QqBotTransport::Websocket => Ok(ConfiguredGateway::QqWebsocket(QqBotWebsocketConfig {
            base_url,
            app_id: credentials.app_id,
            client_secret: credentials.client_secret,
            verbose,
        })),
        QqBotTransport::Webhook => Ok(ConfiguredGateway::QqWebhook(QqBotWebhookServerConfig {
            listen: parse_listen_addr(&qq.listen).unwrap_or(default_qq_listen_addr()),
            base_url,
            app_id: credentials.app_id,
            client_secret: credentials.client_secret,
            verbose,
        })),
    }
}

/// 组装微信 iLink 网关配置。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `weixin`: 微信渠道配置
/// - `verbose`: 是否输出详细日志
///
/// 返回:
/// - 已配置微信网关
fn configured_weixin_gateway(
    paths: &SaiPaths,
    weixin: &WeixinGatewayConfig,
    verbose: bool,
) -> Result<ConfiguredGateway> {
    let base_url =
        non_empty_config(&weixin.base_url).unwrap_or_else(|| default_weixin_base_url().to_string());
    let cdn_base_url = non_empty_config(&weixin.cdn_base_url)
        .unwrap_or_else(|| default_weixin_cdn_base_url().to_string());
    let token = non_empty_config(&weixin.token);
    let account = non_empty_config(&weixin.account);
    let (base_url, cdn_base_url, token) = if let Some(token) = token {
        (base_url, cdn_base_url, token)
    } else {
        let account = load_weixin_account(paths, account.as_deref())?;
        (account.base_url, account.cdn_base_url, account.token)
    };
    Ok(ConfiguredGateway::Weixin(WeixinBotServerConfig {
        base_url,
        cdn_base_url,
        token,
        bot_agent: non_empty_config(&weixin.bot_agent),
        verbose,
    }))
}

/// 启动单个渠道任务。
///
/// 参数:
/// - `tasks`: 任务集合
/// - `paths`: Sai 路径
/// - `gateway`: 渠道网关配置
///
/// 返回:
/// - 无
fn spawn_gateway(tasks: &mut JoinSet<Result<()>>, paths: SaiPaths, gateway: ConfiguredGateway) {
    tasks.spawn(async move {
        match gateway {
            ConfiguredGateway::QqWebsocket(config) => run_qq_bot_websocket(&paths, config).await,
            ConfiguredGateway::QqWebhook(config) => run_qq_bot_webhook_server(&paths, config).await,
            ConfiguredGateway::Weixin(config) => run_weixin_bot_server(&paths, config).await,
        }
    });
}

/// 读取非空配置字符串。
///
/// 参数:
/// - `value`: 配置字符串
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

/// 解析监听地址。
///
/// 参数:
/// - `value`: 监听地址文本
///
/// 返回:
/// - Socket 地址
fn parse_listen_addr(value: &str) -> Result<SocketAddr> {
    value.trim().parse::<SocketAddr>().with_context(|| {
        format!(
            "{}: {value}",
            t("invalid gateway listen address", "无效网关监听地址")
        )
    })
}

/// 返回 QQ Webhook 默认监听地址。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 默认 Socket 地址
fn default_qq_listen_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8766))
}

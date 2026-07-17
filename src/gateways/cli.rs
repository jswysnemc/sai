use super::message::{MediaKind, OutboundMedia, OutboundMessage};
use super::onebot::{OneBotClient, OneBotTargetKind};
use super::onebot_server::{run_onebot_server, OneBotServerConfig};
use super::qq_bot::config::{
    resolve_qq_credentials, resolve_qq_transport, QqBotCredentialOverrides, QqBotTransport,
};
use super::qq_bot::webhook_server::{run_qq_bot_webhook_server, QqBotWebhookServerConfig};
use super::qq_bot::websocket::{run_qq_bot_websocket, QqBotWebsocketConfig};
use super::qq_official::{QqOfficialClient, QqTargetKind};
use super::supervisor::run_configured_gateways;
use super::wecom_webhook::WecomWebhookClient;
use super::weixin_bot::client::default_cdn_base_url as default_weixin_cdn_base_url;
use super::weixin_bot::login::{
    default_base_url as default_weixin_base_url, default_bot_type as default_weixin_bot_type,
    load_weixin_account, run_weixin_login, WeixinLoginConfig,
};
use super::weixin_bot::server::{run_weixin_bot_server, WeixinBotServerConfig};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use serde_json::Value;
use std::net::SocketAddr;
use std::path::PathBuf;

const LEGACY_WEIXIN_BASE_URL: &str = "https://ilink.tencentbot.top";
const LEGACY_WEIXIN_BOT_TYPE: &str = "WeChat";

#[derive(Debug, Args)]
pub(crate) struct GatewayArgs {
    #[arg(long, short = 'v', global = true)]
    pub(crate) verbose: bool,
    #[command(subcommand)]
    pub(crate) command: GatewayCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum GatewayCommand {
    WecomWebhook(WecomWebhookArgs),
    Start(GatewayStartArgs),
    QqOfficial(QqOfficialArgs),
    QqBot(QqBotArgs),
    QqBotWebhook(QqBotWebhookArgs),
    Onebot(OneBotArgs),
    OnebotServer(OneBotServerArgs),
    WeixinLogin(WeixinLoginArgs),
    WeixinServer(WeixinServerArgs),
    #[command(hide = true)]
    Scheduler,
}

#[derive(Debug, Args)]
pub(crate) struct GatewayStartArgs {}

#[derive(Debug, Args)]
pub(crate) struct WecomWebhookArgs {
    #[arg(long)]
    pub(crate) webhook_url: String,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long)]
    pub(crate) image: Vec<PathBuf>,
    #[arg(long)]
    pub(crate) file: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct QqOfficialArgs {
    #[arg(long, default_value = "https://api.sgroup.qq.com")]
    pub(crate) base_url: String,
    #[arg(long)]
    pub(crate) authorization: String,
    #[arg(long)]
    pub(crate) target_kind: String,
    #[arg(long)]
    pub(crate) target_id: String,
    #[arg(long)]
    pub(crate) msg_id: Option<String>,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long)]
    pub(crate) image: Vec<PathBuf>,
    #[arg(long)]
    pub(crate) file: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct QqBotArgs {
    #[arg(long)]
    pub(crate) transport: Option<String>,
    #[arg(long)]
    pub(crate) listen: Option<SocketAddr>,
    #[arg(long)]
    pub(crate) base_url: Option<String>,
    #[arg(long)]
    pub(crate) token: Option<String>,
    #[arg(long)]
    pub(crate) app_id: Option<String>,
    #[arg(long)]
    pub(crate) client_secret: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct QqBotWebhookArgs {
    #[arg(long)]
    pub(crate) listen: Option<SocketAddr>,
    #[arg(long)]
    pub(crate) base_url: Option<String>,
    #[arg(long)]
    pub(crate) token: Option<String>,
    #[arg(long)]
    pub(crate) app_id: Option<String>,
    #[arg(long)]
    pub(crate) client_secret: Option<String>,
}

impl From<QqBotWebhookArgs> for QqBotArgs {
    /// 将旧 Webhook 命令参数转换为通用 QQ 官方机器人参数。
    ///
    /// 参数:
    /// - `value`: 旧 Webhook 命令参数
    ///
    /// 返回:
    /// - 通用 QQ 官方机器人参数
    fn from(value: QqBotWebhookArgs) -> Self {
        Self {
            transport: None,
            listen: value.listen,
            base_url: value.base_url,
            token: value.token,
            app_id: value.app_id,
            client_secret: value.client_secret,
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct OneBotArgs {
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    pub(crate) base_url: String,
    #[arg(long)]
    pub(crate) access_token: Option<String>,
    #[arg(long)]
    pub(crate) target_kind: String,
    #[arg(long)]
    pub(crate) target_id: i64,
    #[arg(long)]
    pub(crate) text: Option<String>,
    #[arg(long)]
    pub(crate) image: Vec<PathBuf>,
    #[arg(long)]
    pub(crate) file: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub(crate) struct OneBotServerArgs {
    #[arg(long, default_value = "127.0.0.1:8765")]
    pub(crate) listen: SocketAddr,
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    pub(crate) onebot_base_url: String,
    #[arg(long)]
    pub(crate) access_token: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct WeixinServerArgs {
    #[arg(long)]
    pub(crate) base_url: Option<String>,
    #[arg(long)]
    pub(crate) cdn_base_url: Option<String>,
    #[arg(long)]
    pub(crate) token: Option<String>,
    #[arg(long)]
    pub(crate) account: Option<String>,
    #[arg(long)]
    pub(crate) bot_agent: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct WeixinLoginArgs {
    #[arg(long)]
    pub(crate) base_url: Option<String>,
    #[arg(long)]
    pub(crate) bot_type: Option<String>,
    #[arg(long, default_value_t = 480)]
    pub(crate) timeout_secs: u64,
}

/// 运行消息网关命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: 网关命令参数
///
/// 返回:
/// - 命令是否成功
pub(crate) async fn run_gateway(paths: &SaiPaths, args: GatewayArgs) -> Result<()> {
    let verbose = args.verbose;
    enter_gateway_workspace(paths, verbose)?;
    let responses = match args.command {
        GatewayCommand::Start(_args) => {
            run_configured_gateways(paths, verbose).await?;
            return Ok(());
        }
        GatewayCommand::Scheduler => {
            crate::cron::run_scheduler(paths.clone()).await?;
            return Ok(());
        }
        GatewayCommand::WecomWebhook(args) => {
            let message = outbound_message(args.text, args.image, args.file)?;
            let client = WecomWebhookClient::new(args.webhook_url);
            client.send(&message).await?
        }
        GatewayCommand::QqOfficial(args) => {
            let message = outbound_message(args.text, args.image, args.file)?;
            let target_kind = QqTargetKind::parse(&args.target_kind)?;
            let client = QqOfficialClient::new(
                args.base_url,
                args.authorization,
                target_kind,
                args.target_id,
            );
            client.send(&message, args.msg_id.as_deref()).await?
        }
        GatewayCommand::QqBot(args) => {
            run_qq_bot_gateway(paths, args, None, verbose).await?;
            return Ok(());
        }
        GatewayCommand::QqBotWebhook(args) => {
            run_qq_bot_gateway(paths, args.into(), Some(QqBotTransport::Webhook), verbose).await?;
            return Ok(());
        }
        GatewayCommand::Onebot(args) => {
            let message = outbound_message(args.text, args.image, args.file)?;
            let target_kind = OneBotTargetKind::parse(&args.target_kind)?;
            let client = OneBotClient::new(
                args.base_url,
                args.access_token,
                target_kind,
                args.target_id,
            );
            client.send(&message).await?
        }
        GatewayCommand::OnebotServer(args) => {
            run_onebot_server(
                paths,
                OneBotServerConfig {
                    listen: args.listen,
                    onebot_base_url: args.onebot_base_url,
                    access_token: args.access_token,
                },
            )
            .await?;
            return Ok(());
        }
        GatewayCommand::WeixinLogin(args) => {
            let config = load_gateway_config(paths)?;
            let weixin = &config.gateways.weixin;
            let (base_url, bot_type) = resolve_weixin_login_settings(
                args.base_url,
                args.bot_type,
                &weixin.base_url,
                &weixin.bot_type,
            );
            run_weixin_login(
                paths,
                WeixinLoginConfig {
                    base_url,
                    bot_type,
                    timeout_secs: args.timeout_secs,
                },
            )
            .await?;
            return Ok(());
        }
        GatewayCommand::WeixinServer(args) => {
            let config = load_gateway_config(paths)?;
            let weixin = &config.gateways.weixin;
            let base_url = non_empty_arg(args.base_url, &weixin.base_url)
                .unwrap_or_else(|| default_weixin_base_url().to_string());
            let cdn_base_url = non_empty_arg(args.cdn_base_url, &weixin.cdn_base_url)
                .unwrap_or_else(|| default_weixin_cdn_base_url().to_string());
            let token_arg = non_empty_arg(args.token, &weixin.token);
            let account = non_empty_arg(args.account, &weixin.account);
            let (base_url, cdn_base_url, token) = if let Some(token) = token_arg {
                (base_url, cdn_base_url, token)
            } else {
                let account = load_weixin_account(paths, account.as_deref())?;
                (account.base_url, account.cdn_base_url, account.token)
            };
            run_weixin_bot_server(
                paths,
                WeixinBotServerConfig {
                    base_url,
                    cdn_base_url,
                    token,
                    bot_agent: non_empty_arg(args.bot_agent, &weixin.bot_agent),
                    verbose,
                },
            )
            .await?;
            return Ok(());
        }
    };
    println!("{}", serde_json::to_string_pretty(&responses)?);
    Ok(())
}

/// 解析微信登录参数，并兼容已经写入配置文件的失效旧默认值。
///
/// 参数:
/// - `base_url`: 命令行指定的 API 基础地址
/// - `bot_type`: 命令行指定的机器人类型
/// - `configured_base_url`: 配置文件中的 API 基础地址
/// - `configured_bot_type`: 配置文件中的机器人类型
///
/// 返回:
/// - 当前可用的 API 基础地址和机器人类型
fn resolve_weixin_login_settings(
    base_url: Option<String>,
    bot_type: Option<String>,
    configured_base_url: &str,
    configured_bot_type: &str,
) -> (String, String) {
    // 1. 解析 API 地址并替换失效的旧默认值
    let base_url = resolve_weixin_login_setting(
        base_url,
        configured_base_url,
        LEGACY_WEIXIN_BASE_URL,
        default_weixin_base_url(),
    );
    // 2. 解析机器人类型并替换失效的旧默认值
    let bot_type = resolve_weixin_login_setting(
        bot_type,
        configured_bot_type,
        LEGACY_WEIXIN_BOT_TYPE,
        default_weixin_bot_type(),
    );
    (base_url, bot_type)
}

/// 解析单项微信登录参数，并优先采用命令行显式值。
///
/// 参数:
/// - `explicit`: 命令行显式值
/// - `configured`: 配置文件值
/// - `legacy`: 需要替换的失效旧值
/// - `default`: 当前默认值
///
/// 返回:
/// - 解析后的有效参数
fn resolve_weixin_login_setting(
    explicit: Option<String>,
    configured: &str,
    legacy: &str,
    default: &str,
) -> String {
    // 1. 命令行显式值始终优先
    if let Some(explicit) = non_empty_arg(explicit, "") {
        return explicit;
    }
    // 2. 配置为空或仍为失效旧值时使用当前默认值
    let configured = configured.trim();
    if configured.is_empty() || configured.eq_ignore_ascii_case(legacy) {
        default.to_string()
    } else {
        configured.to_string()
    }
}

/// 读取网关需要的应用配置。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 应用配置
fn load_gateway_config(paths: &SaiPaths) -> Result<AppConfig> {
    AppConfig::init_files(paths)?;
    AppConfig::load_or_default(paths)
}

/// 运行 QQ 官方机器人网关。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: QQ 官方机器人命令行参数
/// - `forced_transport`: 强制传输模式
/// - `verbose`: 是否输出详细日志
///
/// 返回:
/// - 网关运行结果
async fn run_qq_bot_gateway(
    paths: &SaiPaths,
    args: QqBotArgs,
    forced_transport: Option<QqBotTransport>,
    verbose: bool,
) -> Result<()> {
    let config = load_gateway_config(paths)?;
    let qq = &config.gateways.qq;
    let credentials = resolve_qq_credentials(
        QqBotCredentialOverrides {
            token: args.token.as_deref(),
            app_id: args.app_id.as_deref(),
            client_secret: args.client_secret.as_deref(),
        },
        qq,
    )?;
    let base_url = non_empty_arg(args.base_url.clone(), &qq.base_url)
        .unwrap_or_else(|| "https://api.sgroup.qq.com".to_string());
    let transport = match forced_transport {
        Some(transport) => transport,
        None => resolve_qq_transport(args.transport.as_deref(), &qq.transport)?,
    };
    match transport {
        QqBotTransport::Websocket => {
            run_qq_bot_websocket(
                paths,
                QqBotWebsocketConfig {
                    base_url,
                    app_id: credentials.app_id,
                    client_secret: credentials.client_secret,
                    verbose,
                },
            )
            .await
        }
        QqBotTransport::Webhook => {
            run_qq_bot_webhook_server(
                paths,
                QqBotWebhookServerConfig {
                    listen: args
                        .listen
                        .or_else(|| parse_listen_addr(&qq.listen).ok())
                        .unwrap_or(default_qq_listen_addr()),
                    base_url,
                    app_id: credentials.app_id,
                    client_secret: credentials.client_secret,
                    verbose,
                },
            )
            .await
        }
    }
}

/// 读取命令行参数或配置中的非空字符串。
///
/// 参数:
/// - `arg`: 命令行参数
/// - `configured`: TUI 保存的配置值
///
/// 返回:
/// - 优先使用命令行参数，其次使用配置值
fn non_empty_arg(arg: Option<String>, configured: &str) -> Option<String> {
    arg.filter(|value| !value.trim().is_empty()).or_else(|| {
        let configured = configured.trim();
        if configured.is_empty() {
            None
        } else {
            Some(configured.to_string())
        }
    })
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

/// 切换到网关默认工作目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `verbose`: 是否输出详细日志
///
/// 返回:
/// - 工作目录是否切换成功
fn enter_gateway_workspace(paths: &SaiPaths, verbose: bool) -> Result<()> {
    let workspace = paths.data_dir.join("workspace");
    std::fs::create_dir_all(&workspace)?;
    std::env::set_current_dir(&workspace)?;
    if verbose {
        eprintln!(
            "{}{}",
            t("【Gateway】【Workspace】", "【网关】【工作目录】"),
            workspace.display()
        );
    }
    Ok(())
}

/// 从命令行参数组装统一出站消息。
///
/// 参数:
/// - `text`: 文本消息
/// - `images`: 图片路径列表
/// - `files`: 文件路径列表
///
/// 返回:
/// - 统一出站消息
fn outbound_message(
    text: Option<String>,
    images: Vec<PathBuf>,
    files: Vec<PathBuf>,
) -> Result<OutboundMessage> {
    let mut message = OutboundMessage {
        text: text.filter(|value| !value.trim().is_empty()),
        media: Vec::new(),
    };
    for path in images {
        validate_file(&path)?;
        message.media.push(OutboundMedia {
            kind: MediaKind::Image,
            path,
        });
    }
    for path in files {
        validate_file(&path)?;
        message.media.push(OutboundMedia {
            kind: MediaKind::File,
            path,
        });
    }
    if message.is_empty() {
        bail!(t(
            "provide --text, --image, or --file",
            "请提供 --text、--image 或 --file"
        ));
    }
    Ok(message)
}

/// 校验待发送文件存在且不是目录。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 文件是否有效
fn validate_file(path: &PathBuf) -> Result<()> {
    let metadata = std::fs::metadata(path).map_err(|err| {
        anyhow::anyhow!(
            "{} {}: {err}",
            t("invalid media path", "无效媒体路径"),
            path.display()
        )
    })?;
    if !metadata.is_file() {
        bail!(
            "{}: {}",
            t("media path is not a file", "媒体路径不是文件"),
            path.display()
        );
    }
    Ok(())
}

#[allow(dead_code)]
fn _responses_are_json(_responses: &[Value]) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateways::qq_bot::config::parse_qq_token;

    #[test]
    fn rejects_empty_outbound_message() {
        let err = outbound_message(None, Vec::new(), Vec::new()).unwrap_err();

        assert!(err.to_string().contains(t(
            "provide --text, --image, or --file",
            "请提供 --text、--image 或 --file"
        )));
    }

    #[test]
    fn parses_qq_token_credentials() {
        let credentials = parse_qq_token("1903262889:secret-value").unwrap();

        assert_eq!(credentials.app_id, "1903262889");
        assert_eq!(credentials.client_secret, "secret-value");
    }

    #[test]
    fn rejects_invalid_qq_token_credentials() {
        let err = parse_qq_token("1903262889").unwrap_err();

        assert!(err.to_string().contains("AppID:AppSecret"));
    }

    /// 验证失效的微信旧默认配置会回退到当前官方参数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn resolves_legacy_weixin_login_defaults() {
        let (base_url, bot_type) =
            resolve_weixin_login_settings(None, None, "https://ilink.tencentbot.top", "WeChat");

        assert_eq!(base_url, "https://ilinkai.weixin.qq.com");
        assert_eq!(bot_type, "3");
    }
}

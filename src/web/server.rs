use super::api;
use super::app_state::WebAppState;
use super::assets;
use super::bind_address;
use super::runs::RunManager;
use super::services::weixin_login::WeixinLoginManager;
use super::system_monitor::SystemMonitor;
use super::terminal::TerminalManager;
use super::workspaces::WorkspaceManager;
use crate::cli::WebArgs;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use std::sync::Arc;

/// 启动 Axum Web 服务并等待退出信号。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: Web 服务参数
///
/// 返回:
/// - 服务运行结果
pub(super) async fn run(paths: &SaiPaths, args: WebArgs) -> Result<()> {
    AppConfig::init_files(paths)?;
    let token = generate_token();
    let secrets = crate::config::SecretsConfig::load(paths)?;
    let password_hash = secrets
        .web_password_hash
        .as_deref()
        .map(|hash| Arc::from(hash) as Arc<str>);
    let workspaces = WorkspaceManager::new(paths, args.workspace.as_deref())?;
    let runs = RunManager::new(paths)?;
    let state = WebAppState {
        paths: paths.clone(),
        auth_token: Arc::from(token.as_str()),
        password_hash: password_hash.clone(),
        workspaces,
        runs: runs.clone(),
        terminals: TerminalManager::new(),
        system_monitor: SystemMonitor::new(),
        weixin_login: WeixinLoginManager::new(paths),
    };
    runs.resume_queued().await;
    let app = Router::new()
        .merge(api::router(state.clone()))
        .fallback(assets::serve)
        .with_state(state);
    let address = bind_address::resolve_bind_address(&args.host, args.port)?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind Sai Web at {address}"))?;
    let address = listener.local_addr()?;
    let url = bind_address::browsable_url(&address, &token);

    // 通配监听下 URL 里的主机部分不能直接用于远程访问，需说明如何替换。
    // 不去猜测对外地址：多网卡与 VPN 环境下默认路由未必是用户要用的那条网络
    if bind_address::is_wildcard(&address) {
        println!("Sai Web: listening on {address} (all interfaces)");
        println!("  Local:  {url}");
        println!(
            "  Remote: {}",
            bind_address::url_for_host("HOST", address.port(), &token)
        );
        println!("  Replace HOST with this machine's address on the network you connect from.");
    } else {
        println!("Sai Web: {url}");
    }

    // 绑定到非回环地址意味着同网段任何人都可访问，没有口令时仅凭令牌泄露即可接管
    if bind_address::is_externally_reachable(&address) && password_hash.is_none() {
        eprintln!(
            "Warning: Sai Web is reachable from other machines on {address} without a password. \
             Set one with `sai web-password set`, or bind to 127.0.0.1."
        );
    }

    if !args.no_open {
        let _ = open::that_detached(&url);
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// 生成单次服务启动令牌。
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// 等待 Ctrl+C 退出信号。
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

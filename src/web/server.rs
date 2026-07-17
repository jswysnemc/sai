use super::api;
use super::app_state::WebAppState;
use super::assets;
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
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
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
    let runs = RunManager::new(paths)?;
    let state = WebAppState {
        paths: paths.clone(),
        auth_token: Arc::from(token.as_str()),
        workspaces: WorkspaceManager::new(paths)?,
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
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), args.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind Sai Web at {address}"))?;
    let address = listener.local_addr()?;
    let url = format!("http://{address}/?token={token}");
    println!("Sai Web: {url}");
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

mod api;
mod app_state;
mod assets;
mod auth;
mod bind_address;
mod browser;
mod daemon;
mod error;
pub(crate) mod password;
// 会话事件总线（src/runner/session_actor.rs）需要复用 Web 事件与事件日志类型
pub(crate) mod runs;
mod server;
mod server_logging;
mod server_shutdown;
pub(crate) mod services;
pub(crate) mod ssh;
mod system_monitor;
mod terminal;
mod workspace;
mod workspaces;
pub(crate) use workspace::{git_diff, git_file_stats, git_status, GitFileStats};

use crate::cli::WebArgs;
use crate::paths::SaiPaths;
use anyhow::Result;

/// 启动 Sai Web 编程工作台。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: Web 服务启动参数
///
/// 返回:
/// - 服务运行结果
pub(crate) async fn run(paths: &SaiPaths, args: WebArgs) -> Result<()> {
    let update = args.command.is_some();
    let directory = assets::distribution::ensure(paths, update).await?;
    if update {
        println!(
            "Sai Web {}: {}",
            env!("CARGO_PKG_VERSION"),
            directory.display()
        );
        return Ok(());
    }
    if daemon::detach_if_interactive(&args)? {
        return Ok(());
    }
    server::run(paths, args, directory).await
}

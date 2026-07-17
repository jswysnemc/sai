use super::runs::RunManager;
use super::services::weixin_login::WeixinLoginManager;
use super::system_monitor::SystemMonitor;
use super::terminal::TerminalManager;
use super::workspaces::WorkspaceManager;
use crate::paths::SaiPaths;
use std::sync::Arc;

/// Web 路由共享依赖。
#[derive(Clone)]
pub(super) struct WebAppState {
    pub paths: SaiPaths,
    pub auth_token: Arc<str>,
    pub workspaces: WorkspaceManager,
    pub runs: RunManager,
    pub terminals: TerminalManager,
    pub system_monitor: SystemMonitor,
    pub weixin_login: WeixinLoginManager,
}

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
    /// Web 访问口令的 Argon2 哈希；为空表示只用启动令牌验证
    pub password_hash: Option<Arc<str>>,
    /// 为真时本机回环访问不校验启动令牌
    pub allow_anonymous: bool,
    pub workspaces: WorkspaceManager,
    pub runs: RunManager,
    pub terminals: TerminalManager,
    pub system_monitor: SystemMonitor,
    pub weixin_login: WeixinLoginManager,
}

use crate::config::AppConfig;
use crate::paths::SaiPaths;

/// 【终端轮次】【会话上下文】自动唤醒与队列执行共用配置和会话所有权信息。
#[derive(Clone, Copy)]
pub(super) struct ReplTurnContext<'a> {
    pub paths: &'a SaiPaths,
    pub config: &'a AppConfig,
    pub owner_key: &'a str,
}

use super::{session_key, RunManager};
use crate::runner::{ActorHandle, SessionActor, SessionOwner, SessionPresenceGuard};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

/// 保存会话级事件总线。
///
/// 生命周期与会话一致：运行检查点淘汰不得回收会话日志，否则刷新页面后
/// 无法从磁盘补发同一会话的历史事件。
#[derive(Default)]
pub(super) struct SessionBuses {
    pub(super) entries: HashMap<String, ActorHandle>,
    /// 【Web】【会话在线】仅供网格探测，不转交运行或选举持有者
    presence: HashMap<String, SessionPresenceGuard>,
    /// 【Web】【事件日志】持有订阅守卫，删除会话时一并取消后台任务
    pub(super) console_logs: HashMap<String, crate::web::server_logging::ConsoleSubscription>,
}

impl RunManager {
    /// 返回指定会话的事件总线，不存在时从磁盘恢复并启动。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 会话事件总线句柄
    pub(crate) async fn session_bus(&self, workspace_id: &str, session_id: &str) -> ActorHandle {
        let key = session_key(workspace_id, session_id);
        if let Some(bus) = self.buses.read().await.entries.get(&key) {
            return bus.clone();
        }
        let mut buses = self.buses.write().await;
        if let Some(bus) = buses.entries.get(&key) {
            return bus.clone();
        }
        let journal_path = self.session_event_path(&key);
        let bus = SessionActor::spawn(journal_path, workspace_id, session_id);
        if let Ok((_, state_dir)) = crate::state::locate_session_dirs(&self.paths, session_id) {
            if let Ok(presence) =
                SessionPresenceGuard::register(&state_dir, session_id, SessionOwner::Web)
            {
                buses.presence.insert(key.clone(), presence);
            }
        }
        if self.console_logging {
            if let Some(subscription) = crate::web::server_logging::subscribe_runs(&bus) {
                buses.console_logs.insert(key.clone(), subscription);
            }
        }
        buses.entries.insert(key, bus.clone());
        bus
    }

    /// 返回指定运行所属会话的事件总线。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    ///
    /// 返回:
    /// - 会话事件总线；运行检查点不存在时返回空
    pub(crate) async fn run_bus(&self, run_id: &str) -> Option<ActorHandle> {
        let checkpoint = self.checkpoints.get(run_id)?;
        Some(
            self.session_bus(&checkpoint.info.workspace_id, &checkpoint.info.session_id)
                .await,
        )
    }

    /// 删除指定会话的运行检查点、会话事件总线与磁盘日志。
    ///
    /// 参数:
    /// - `workspace_id`: 会话所属工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 清理结果
    pub(crate) async fn remove_session_history(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> Result<()> {
        let removed = self.checkpoints.remove_session(workspace_id, session_id)?;
        if removed.is_empty() {
            return Ok(());
        }
        let key = session_key(workspace_id, session_id);
        // 释放句柄即关闭命令通道，事件总线任务随之退出
        let mut buses = self.buses.write().await;
        buses.console_logs.remove(&key);
        buses.entries.remove(&key);
        buses.presence.remove(&key);
        drop(buses);
        let path = self.session_event_path(&key);
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    /// 返回会话事件日志路径。
    ///
    /// 参数:
    /// - `key`: 工作区会话级调度键
    ///
    /// 返回:
    /// - JSONL 事件文件路径
    pub(super) fn session_event_path(&self, key: &str) -> PathBuf {
        let (workspace_id, session_id) = match key.split_once(':') {
            Some(pair) => pair,
            None => ("", key),
        };
        crate::web::runs::session_event_path(&self.paths.state_dir, workspace_id, session_id)
    }
}

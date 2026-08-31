use super::{session_key, RunManager};
use crate::ipc::link::{HolderRequest, SessionLink};
use crate::runner::{ActorHandle, SessionActor, SessionOwner};
use anyhow::Result;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

/// 保存会话级事件总线。
///
/// 生命周期与会话一致：运行检查点淘汰不得回收会话日志，否则刷新页面后
/// 无法从磁盘补发同一会话的历史事件。
#[derive(Default)]
pub(super) struct SessionBuses {
    pub(super) entries: HashMap<String, ActorHandle>,
    /// 会话的跨进程链接；与事件总线同生命周期，便于诊断当前角色。
    pub(super) links: HashMap<String, SessionLink>,
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
        let (link, bus) = spawn_session_bus(self, workspace_id, session_id, journal_path).await;
        buses.links.insert(key.clone(), link);
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
        buses.entries.remove(&key);
        buses.links.remove(&key);
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

/// 建立会话事件总线：先尝试跨进程链接，拿不到端点时退回本地总线。
///
/// 跨进程链接会决定本进程是该会话的持有者还是观察者：
/// 持有者写会话事件文件并代跑观察者上行的一轮，观察者把自己的事件与提交
/// 上行给持有者。任何一步失败都退回 [`SessionActor::spawn`]，语义与 P3 完全一致。
///
/// 返回的 future 显式装箱成 `Send`：持有者侧要在专属任务里执行观察者上行的
/// 一轮（见 [`RunManager::serve_holder_requests`]），那条链路会一路 await 到这里；
/// 靠 `async fn` 的自动推导时 rustc 无法穿过这么多层不透明 future 证明 `Send`。
///
/// 参数:
/// - `manager`: 运行管理器，登记为持有者侧的执行器
/// - `workspace_id`: 工作区标识
/// - `session_id`: 会话标识
/// - `journal_path`: 会话事件日志文件
///
/// 返回:
/// - （会话链接，事件总线句柄）
fn spawn_session_bus<'a>(
    manager: &'a RunManager,
    workspace_id: &'a str,
    session_id: &'a str,
    journal_path: PathBuf,
) -> Pin<Box<dyn Future<Output = (SessionLink, ActorHandle)> + Send + 'a>> {
    Box::pin(async move {
        // 会话状态目录决定了 IPC 端点；定位不到就只能是单进程模式
        let Ok((_, state_dir)) = crate::state::locate_session_dirs(&manager.paths, session_id)
        else {
            return (
                SessionLink::detached(&journal_path, workspace_id, session_id),
                SessionActor::spawn(journal_path, workspace_id, session_id),
            );
        };
        let (link, bus) = SessionLink::attach(
            SessionOwner::Web,
            &state_dir,
            session_id,
            journal_path.clone(),
            workspace_id,
        )
        .await;
        // 登记执行器与角色无关：接管后本进程可能从观察者变持有者，
        // 执行器必须提前就位，否则接管后观察者上行的一轮只会被拒绝
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<HolderRequest>();
        link.set_holder_sink(tx);
        tokio::spawn(manager.clone().serve_holder_requests(rx));
        (link, bus)
    })
}

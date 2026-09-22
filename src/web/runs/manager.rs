use self::history::SessionBuses;
use self::message_queue::WebMessageQueue;
use super::checkpoint::{RunCheckpoint, RunCheckpointStatus, RunCheckpointStore};
use super::model_override::resolve_run_config;
use super::request_limits::validate_start_request;
use super::{EventJournal, WebEvent};
use crate::agent::{AgentMode, InterMessageSource};
use crate::paths::SaiPaths;
use crate::runner::{
    ActorHandle, ControlSubmission, RunnerSubmission, SessionRunner, SubmissionSource,
    UserInputSubmission,
};
use crate::web::workspaces::WorkspaceInfo;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;

mod execution;
mod goal_continuation;
mod history;
mod model;
mod shutdown;
pub(crate) use model::{ActiveRunInfo, QueueInsertAt, RunKind, StartRunRequest};
mod message_queue;
mod queue;
#[cfg(test)]
mod tests;

pub(crate) use queue::QueuedRunUpdate;

struct ActiveRun {
    info: ActiveRunInfo,
    handle: JoinHandle<()>,
    /// 停止请求标志，abort 前置位以便轮次按中断而非失败落库
    cancel_requested: Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Clone)]
struct QueuedRun {
    info: ActiveRunInfo,
    workspace: WorkspaceInfo,
    request: StartRunRequest,
}

/// 管理 Web 运行互斥、事件日志和中断句柄。
#[derive(Clone)]
pub(crate) struct RunManager {
    paths: SaiPaths,
    active: Arc<Mutex<HashMap<String, ActiveRun>>>,
    queued: Arc<Mutex<HashMap<String, VecDeque<QueuedRun>>>>,
    scheduling: Arc<Mutex<()>>,
    /// 会话级事件总线；同一会话的多个前端共享同一份事件流
    buses: Arc<RwLock<SessionBuses>>,
    checkpoints: RunCheckpointStore,
    /// 服务关闭后拒绝新请求及自动启动下一项
    shutting_down: Arc<std::sync::atomic::AtomicBool>,
    /// 仅服务进程订阅控制台日志，TUI 共用管理器时不输出
    console_logging: bool,
}

impl RunManager {
    /// 创建空运行管理器。
    pub(crate) fn new(paths: &SaiPaths) -> Result<Self> {
        let checkpoints = RunCheckpointStore::new(paths)?;
        let mut queued = HashMap::<String, VecDeque<QueuedRun>>::new();
        for checkpoint in checkpoints.queued() {
            queued
                .entry(session_key(
                    &checkpoint.info.workspace_id,
                    &checkpoint.info.session_id,
                ))
                .or_default()
                .push_back(QueuedRun {
                    info: checkpoint.info,
                    workspace: checkpoint.workspace,
                    request: checkpoint.request,
                });
        }
        let manager = Self {
            paths: paths.clone(),
            active: Arc::new(Mutex::new(HashMap::new())),
            queued: Arc::new(Mutex::new(queued)),
            scheduling: Arc::new(Mutex::new(())),
            buses: Arc::new(RwLock::new(SessionBuses::default())),
            checkpoints,
            shutting_down: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            console_logging: false,
        };
        // 【Web】【启动恢复】只恢复没有真实活动轮次的检查点，打开界面不占用会话
        for checkpoint in
            manager
                .checkpoints
                .recover_running_as_interrupted_where(|checkpoint| {
                    !session_has_active_run(paths, &checkpoint.info.session_id)
                })?
        {
            if let Ok(state) = crate::state::StateStore::for_workspace_session(
                paths,
                std::path::Path::new(&checkpoint.workspace.path),
                &checkpoint.info.session_id,
            ) {
                let _ = state.recover_stale_turns();
            }
            manager
                .checkpoints
                .update_interruption(&checkpoint.info.run_id, false, None)?;
            // 启动阶段还没有事件总线任务，直接写会话日志；后续订阅会从磁盘回载
            let key = session_key(&checkpoint.info.workspace_id, &checkpoint.info.session_id);
            let journal = EventJournal::persistent(manager.session_event_path(&key));
            journal.publish(WebEvent::new(
                &checkpoint.info.run_id,
                &checkpoint.info.workspace_id,
                &checkpoint.info.session_id,
                "run.interrupted",
                json!({
                    "recovered": true,
                    "discard_user_turn": false,
                    "restore_input": null,
                    "detail": "Sai restarted while this run was still active.",
                }),
            ));
        }
        // 被跳过的那一轮标 orphaned：它不属于本进程，但也不该继续显示为「本进程运行中」
        for checkpoint in manager.checkpoints.running_or_orphaned() {
            if session_has_active_run(paths, &checkpoint.info.session_id) {
                let _ = manager.checkpoints.mark_orphaned(&checkpoint.info.run_id);
            }
        }
        Ok(manager)
    }

    /// 启动一轮 Agent 运行。
    ///
    /// 参数:
    /// - `workspace`: 当前活动工作区
    /// - `request`: 用户输入
    ///
    /// 返回:
    /// - 活动运行摘要
    pub(crate) async fn start(
        &self,
        workspace: WorkspaceInfo,
        request: StartRunRequest,
    ) -> Result<ActiveRunInfo> {
        validate_start_request(&request)?;
        if request.kind == RunKind::Conversation
            && request.input.trim().is_empty()
            && request.image_url.is_none()
            && request.image_urls.is_empty()
        {
            bail!("message cannot be empty");
        }
        AgentMode::parse(request.mode.as_deref())?;
        let _scheduling = self.scheduling.lock().await;
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
            bail!("Web server is shutting down");
        }
        let key = session_key(&workspace.id, &request.session_id);
        let has_active = self.active.lock().await.contains_key(&key);
        let has_queued = self
            .queued
            .lock()
            .await
            .get(&key)
            .is_some_and(|queue| !queue.is_empty());
        let status = if has_active || has_queued {
            RunCheckpointStatus::Queued
        } else {
            RunCheckpointStatus::Running
        };
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let info = ActiveRunInfo {
            run_id: run_id.clone(),
            workspace_id: workspace.id.clone(),
            session_id: request.session_id.clone(),
            input: request.input.clone(),
            image_urls: request
                .image_url
                .clone()
                .into_iter()
                .chain(request.image_urls.clone())
                .collect(),
            status,
            discard_user_turn: false,
            restore_input: None,
            insert_at: request.insert_at,
        };
        let bus = self.session_bus(&workspace.id, &request.session_id).await;
        self.checkpoints.upsert(RunCheckpoint {
            info: info.clone(),
            workspace: workspace.clone(),
            request: request.clone(),
            status,
            updated_at: String::new(),
        })?;
        let queued_run = QueuedRun {
            info: info.clone(),
            workspace,
            request,
        };
        if status == RunCheckpointStatus::Queued {
            let mut queues = self.queued.lock().await;
            let queue = queues.entry(key).or_default();
            queue.push_back(queued_run);
            // 带上输入，后加入的标签页才能仅凭事件流重建排队中的用户气泡
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.queued",
                json!({
                    "position": queue.len(),
                    "input": info.input,
                    "image_urls": info.image_urls,
                    "insert_at": info.insert_at,
                }),
            ));
            return Ok(info);
        }
        self.spawn_run(key, queued_run, bus).await;
        Ok(info)
    }

    /// 恢复进程重启前尚未执行的排队运行。
    pub(crate) async fn resume_queued(&self) {
        let keys = self.queued.lock().await.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            self.launch_next(&key).await;
        }
    }

    /// 返回全部活动运行。
    ///
    /// 返回:
    /// - 活动运行摘要列表
    pub(crate) async fn active_runs(&self) -> Vec<ActiveRunInfo> {
        let mut runs = self
            .active
            .lock()
            .await
            .values()
            .map(|active| active.info.clone())
            .collect::<Vec<_>>();
        runs.extend(
            self.queued
                .lock()
                .await
                .values()
                .flat_map(|queue| queue.iter().map(|run| run.info.clone())),
        );
        runs
    }

    /// 判断指定会话是否存在活动运行。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区 ID
    /// - `session_id`: 会话 ID
    ///
    /// 返回:
    /// - 是否存在活动运行
    pub(crate) async fn is_session_active(&self, workspace_id: &str, session_id: &str) -> bool {
        let key = session_key(workspace_id, session_id);
        self.active.lock().await.contains_key(&key)
            || self
                .queued
                .lock()
                .await
                .get(&key)
                .is_some_and(|queue| !queue.is_empty())
    }

    /// 中断指定运行。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    ///
    /// 返回:
    /// - 是否执行了中断
    pub(crate) async fn stop(&self, run_id: &str) -> Result<bool> {
        let _scheduling = self.scheduling.lock().await;
        let mut active = self.active.lock().await;
        let active_key = active
            .iter()
            .find_map(|(key, run)| (run.info.run_id == run_id).then(|| key.clone()));
        if let Some(key) = active_key {
            let current = active.remove(&key).expect("active run key must exist");
            // 1. 先置位停止标志，再 abort：轮次守卫在析构时读取此标志，
            //    否则用户主动停止会被记成失败
            current
                .cancel_requested
                .store(true, std::sync::atomic::Ordering::SeqCst);
            // 2. 立即 abort，不 await 任务收尾，避免上游 SSE 挂起时 stop API 阻塞
            current.handle.abort();
            let info = current.info.clone();
            drop(active);
            self.checkpoints.update_interruption(run_id, false, None)?;
            let bus = self.session_bus(&info.workspace_id, &info.session_id).await;
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.interrupted",
                json!({
                    "discard_user_turn": false,
                    "restore_input": null,
                    "detail": "The user stopped this run before it completed.",
                }),
            ));
            drop(_scheduling);
            self.launch_next(&key).await;
            return Ok(true);
        }
        drop(active);
        let mut queues = self.queued.lock().await;
        for queue in queues.values_mut() {
            let Some(position) = queue.iter().position(|run| run.info.run_id == run_id) else {
                continue;
            };
            let queued = queue
                .remove(position)
                .expect("queued run position must exist");
            drop(queues);
            self.checkpoints
                .update_interruption(run_id, true, Some(queued.info.input.clone()))?;
            let bus = self
                .session_bus(&queued.info.workspace_id, &queued.info.session_id)
                .await;
            let _ = bus.emit(WebEvent::new(
                &queued.info.run_id,
                &queued.info.workspace_id,
                &queued.info.session_id,
                "run.interrupted",
                json!({
                    "queued": true,
                    "discard_user_turn": true,
                    "restore_input": queued.info.input,
                    "detail": "The queued run was cancelled before it started.",
                }),
            ));
            return Ok(true);
        }
        drop(queues);
        Ok(false)
    }

    /// 取出指定会话尚未消费的无回复中断恢复输入。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 待恢复运行信息，读取后清除恢复标记
    pub(crate) fn take_interruption_recovery(
        &self,
        workspace_id: &str,
        session_id: &str,
    ) -> Result<Option<ActiveRunInfo>> {
        self.checkpoints
            .take_interruption_recovery(workspace_id, session_id)
    }

    /// 清理指定活动运行。
    async fn clear_active_if(&self, key: &str) {
        let mut active = self.active.lock().await;
        active.remove(key);
    }

    /// 启动指定会话队列中的下一项。
    fn launch_next<'a>(&'a self, key: &'a str) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let _scheduling = self.scheduling.lock().await;
            if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst)
                || self.active.lock().await.contains_key(key)
            {
                return;
            }
            let queued = {
                let mut queues = self.queued.lock().await;
                queues.get_mut(key).and_then(VecDeque::pop_front)
            };
            let Some(mut queued) = queued else {
                return;
            };
            queued.info.status = RunCheckpointStatus::Running;
            let _ = self
                .checkpoints
                .update_status(&queued.info.run_id, RunCheckpointStatus::Running);
            let bus = self
                .session_bus(&queued.info.workspace_id, &queued.info.session_id)
                .await;
            let _ = bus.emit(WebEvent::new(
                &queued.info.run_id,
                &queued.info.workspace_id,
                &queued.info.session_id,
                "run.dequeued",
                json!({
                    "input": queued.info.input,
                    "image_urls": queued.info.image_urls,
                }),
            ));
            self.spawn_run(key.to_string(), queued, bus).await;
        })
    }
}

/// 生成工作区会话级调度键。
fn session_key(workspace_id: &str, session_id: &str) -> String {
    format!("{workspace_id}:{session_id}")
}

/// 【Web】【运行检测】只读取轮次锁，旧会话持有者文件不会影响启动恢复。
/// 参数: paths 为应用路径，session_id 为会话标识；返回是否存在活动轮次
fn session_has_active_run(paths: &crate::paths::SaiPaths, session_id: &str) -> bool {
    let Ok((_, state_dir)) = crate::state::locate_session_dirs(paths, session_id) else {
        return false;
    };
    crate::runner::active_run(&state_dir).is_some()
}

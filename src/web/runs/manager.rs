use self::history::SessionBuses;
use self::message_queue::WebMessageQueue;
use super::checkpoint::{RunCheckpoint, RunCheckpointStatus, RunCheckpointStore};
use super::model_override::resolve_run_config;
use super::request_limits::validate_start_request;
use super::{EventJournal, WebEvent};
use crate::agent::{AgentMode, InterMessageSource};
use crate::ipc::link::{HolderRequest, LinkRole, SessionLink, SubmittedRun};
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
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;

mod history;
mod message_queue;
mod queue;
#[cfg(test)]
mod tests;

pub(crate) use queue::QueuedRunUpdate;

/// 排队消息插入当前对话的位置。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QueueInsertAt {
    /// 当前轮下一次模型请求前插入，续在同一轮
    Request,
    /// 本轮结束后作为新一轮插入
    #[default]
    Turn,
}

/// 启动一轮 Web 对话所需参数。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StartRunRequest {
    #[serde(default)]
    pub kind: RunKind,
    pub session_id: String,
    pub input: String,
    // 本地 Agent 选择字段，不会原样进入上游请求。
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    // provider_id/thinking_level 仅由 resolve_run_config 消费；model 用于本地选择，
    // 解析后的模型名会作为 Chat Completions 协议的 model 字段发送。
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking_level: Option<String>,
    /// 排队时插入位置；立即启动的一轮忽略该字段。
    #[serde(default)]
    pub insert_at: QueueInsertAt,
}

/// Web 运行种类。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunKind {
    #[default]
    Conversation,
    Compaction,
    GoalContinuation,
}

/// 活动运行摘要。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ActiveRunInfo {
    pub run_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub input: String,
    pub image_urls: Vec<String>,
    pub status: RunCheckpointStatus,
    #[serde(default)]
    pub discard_user_turn: bool,
    #[serde(default)]
    pub restore_input: Option<String>,
    /// 排队插入点；非排队运行保持默认轮次间隔。
    #[serde(default)]
    pub insert_at: QueueInsertAt,
}

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

/// 观察者上行轮次的跟踪上限。
///
/// 只用来把「停止」路由到持有者，超出这个数量的轮次必然早已结束。
const REMOTE_RUN_TRACK_LIMIT: usize = 64;

/// 管理 Web 运行互斥、事件日志和中断句柄。
#[derive(Clone)]
pub(crate) struct RunManager {
    paths: SaiPaths,
    active: Arc<Mutex<HashMap<String, ActiveRun>>>,
    queued: Arc<Mutex<HashMap<String, VecDeque<QueuedRun>>>>,
    scheduling: Arc<Mutex<()>>,
    /// 会话级事件总线；同一会话的多个前端共享同一份事件流
    buses: Arc<RwLock<SessionBuses>>,
    /// 本进程上行给持有者、由持有者代跑的轮次：（轮次标识，会话调度键）。
    ///
    /// 观察者不写运行检查点（那是对端的所有权），但要记住自己最近发出过哪些
    /// 轮次，否则用户点停止时无从把中断请求路由到持有者。按 FIFO 封顶，
    /// 超出的必然早已结束，不需要精确的完成通知。
    remote_runs: Arc<Mutex<VecDeque<(String, String)>>>,
    checkpoints: RunCheckpointStore,
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
            remote_runs: Arc::new(Mutex::new(VecDeque::new())),
            checkpoints,
        };
        // 启动恢复只处理「没有活着的持有者」的那一轮：持有者还活着说明这一轮
        // 正被另一个进程（TUI 或另一个 sai web 实例）驱动，按崩溃恢复会在对端
        // 毫不知情的情况下把它的轮次标成 interrupted/failed——那才是真正的误杀。
        for checkpoint in
            manager
                .checkpoints
                .recover_running_as_interrupted_where(|checkpoint| {
                    !session_is_held_by_a_live_process(paths, &checkpoint.info.session_id)
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
            if session_is_held_by_a_live_process(paths, &checkpoint.info.session_id) {
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
        self.begin(workspace, request, None).await
    }

    /// 执行一个由其它进程上行的一轮请求。
    ///
    /// 本进程是会话持有者：沿用对端已分配的轮次标识，事件经会话事件总线
    /// 扇出回发起方，发起方凭 run_id 就能对上自己的事件流。
    ///
    /// 参数:
    /// - `workspace`: 轮次所属工作区
    /// - `request`: 用户输入
    /// - `run_id`: 上游已分配的轮次标识
    ///
    /// 返回:
    /// - 活动运行摘要
    pub(crate) async fn start_remote(
        &self,
        workspace: WorkspaceInfo,
        request: StartRunRequest,
        run_id: String,
    ) -> Result<ActiveRunInfo> {
        self.begin(workspace, request, Some(run_id)).await
    }

    /// 发起一轮，可选沿用上游已分配的轮次标识。
    ///
    /// 参数:
    /// - `workspace`: 轮次所属工作区
    /// - `request`: 用户输入
    /// - `run_id`: 上游已分配的轮次标识；为空时本地分配
    ///
    /// 返回:
    /// - 活动运行摘要
    async fn begin(
        &self,
        workspace: WorkspaceInfo,
        request: StartRunRequest,
        run_id: Option<String>,
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
        let run_id = run_id.unwrap_or_else(|| format!("run_{}", uuid::Uuid::new_v4().simple()));
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
        // 观察者不持有 Agent：整包上行给持有者执行，本进程只负责把受理结果回给前端。
        // 持有者与单进程（Detached）都走下面原有的本地路径，行为完全不变。
        if let Some(link) = self.session_link(&workspace.id, &request.session_id).await {
            if link.role() == LinkRole::Observer {
                return self
                    .forward_to_holder(&link, &workspace, request, info)
                    .await;
            }
        }
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

    /// 返回指定会话的跨进程链接。
    ///
    /// 会话事件总线尚未建立时返回空——调用方应先取事件总线。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区标识
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 会话链接
    async fn session_link(&self, workspace_id: &str, session_id: &str) -> Option<SessionLink> {
        self.link_for_key(&session_key(workspace_id, session_id))
            .await
    }

    /// 按调度键返回会话链接。
    ///
    /// 参数:
    /// - `key`: 工作区会话级调度键
    ///
    /// 返回:
    /// - 会话链接；尚未建立时为空
    async fn link_for_key(&self, key: &str) -> Option<SessionLink> {
        self.buses.read().await.links.get(key).cloned()
    }

    /// 把中断请求上行给会话持有者。
    ///
    /// 本进程是观察者时不持有运行表，中断只能交给持有者执行。
    ///
    /// 参数:
    /// - `run_id`: 轮次标识
    /// - `key`: 会话调度键
    ///
    /// 返回:
    /// - 是否执行了中断
    async fn abort_remote(&self, run_id: &str, key: &str) -> Result<bool> {
        let Some(link) = self.link_for_key(key).await else {
            return Ok(false);
        };
        link.abort(run_id).await
    }

    /// 把一轮请求上行给会话持有者。
    ///
    /// 本进程是观察者：自己不跑这一轮，只把参数交给持有者并等回执。
    /// 上行失败一律转成 `Err` 抛给接口层——用户输入绝不能静默消失。
    ///
    /// 参数:
    /// - `link`: 会话链接
    /// - `workspace`: 轮次所属工作区
    /// - `request`: 用户输入
    /// - `info`: 已分配好轮次标识的运行摘要
    ///
    /// 返回:
    /// - 带上持有者侧实际状态的运行摘要
    async fn forward_to_holder(
        &self,
        link: &SessionLink,
        workspace: &WorkspaceInfo,
        request: StartRunRequest,
        info: ActiveRunInfo,
    ) -> Result<ActiveRunInfo> {
        let submitted = SubmittedRun {
            submit_id: format!("sub_{}", uuid::Uuid::new_v4().simple()),
            run_id: info.run_id.clone(),
            workspace_id: workspace.id.clone(),
            workspace_path: workspace.path.clone(),
            request,
        };
        let ack = link.submit(submitted).await?;
        if !ack.accepted {
            bail!(
                "{}",
                ack.reason
                    .unwrap_or_else(|| "会话持有者拒绝了这次提交".to_string())
            );
        }
        let key = session_key(&workspace.id, &info.session_id);
        // 记住这一轮在远端：本进程的 stop 需要把中断请求转发给持有者
        {
            let mut remote = self.remote_runs.lock().await;
            remote.retain(|(run_id, _)| run_id != &info.run_id);
            remote.push_back((info.run_id.clone(), key));
            while remote.len() > REMOTE_RUN_TRACK_LIMIT {
                remote.pop_front();
            }
        }
        let status = match ack.status.as_deref() {
            Some("queued") => RunCheckpointStatus::Queued,
            _ => RunCheckpointStatus::Running,
        };
        Ok(ActiveRunInfo { status, ..info })
    }

    /// 消费持有者侧的观察者请求队列。
    ///
    /// 本进程是会话持有者时，观察者上行的一轮由这里落地执行；观察者请求的
    /// 中断也在这里落到本进程的运行表。
    ///
    /// 参数:
    /// - `rx`: 请求接收端
    ///
    /// 返回:
    /// - 无
    async fn serve_holder_requests(self, mut rx: mpsc::UnboundedReceiver<HolderRequest>) {
        while let Some(request) = rx.recv().await {
            match request {
                HolderRequest::Submit { request, reply } => {
                    let outcome = self.execute_submitted(request).await;
                    let _ = reply.send(outcome);
                }
                HolderRequest::Abort { run_id, reply } => {
                    let outcome = self.stop(&run_id).await;
                    let _ = reply.send(outcome);
                }
            }
        }
    }

    /// 执行一个由观察者上行的一轮。
    ///
    /// 持有者是唯一持有 Agent 的一侧，因此这一轮在本进程跑；产生的事件经
    /// 会话事件总线扇出给全部观察者（本地的与远程的），发起方凭 run_id 对上。
    ///
    /// 参数:
    /// - `request`: 上行的一轮请求
    ///
    /// 返回:
    /// - 该轮在持有者侧的状态（`running` / `queued`）
    async fn execute_submitted(&self, request: SubmittedRun) -> Result<String> {
        let workspace = WorkspaceInfo {
            id: request.workspace_id.clone(),
            name: workspace_name(&request.workspace_path),
            path: request.workspace_path.clone(),
            last_opened_at: String::new(),
        };
        let info = self
            .start_remote(workspace, request.request, request.run_id)
            .await?;
        Ok(match info.status {
            RunCheckpointStatus::Queued => "queued".to_string(),
            RunCheckpointStatus::Running
            | RunCheckpointStatus::Completed
            | RunCheckpointStatus::Failed
            | RunCheckpointStatus::Interrupted
            | RunCheckpointStatus::Orphaned => "running".to_string(),
        })
    }

    /// 启动已经取得会话执行权的运行。
    fn spawn_run(
        &self,
        key: String,
        queued: QueuedRun,
        bus: ActorHandle,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let (start_tx, start_rx) = oneshot::channel();
            let manager = self.clone();
            let task_info = queued.info.clone();
            let inter_message_source: Arc<dyn InterMessageSource> = Arc::new(WebMessageQueue::new(
                self.clone(),
                key.clone(),
                task_info.run_id.clone(),
            ));
            let workspace_path = std::path::PathBuf::from(&queued.workspace.path);
            let paths = self.paths.clone();
            let task_key = key.clone();
            // 标志在 spawn 前创建：stop 需要在任务被 abort 之前置位
            let cancel_requested = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let task_cancel = cancel_requested.clone();
            let handle = tokio::spawn(async move {
                let _ = start_rx.await;
                let terminal = crate::runtime_cwd::scope(
                    workspace_path,
                    run_agent(
                        paths,
                        queued.request,
                        task_info.clone(),
                        bus.clone(),
                        inter_message_source,
                        task_cancel,
                    ),
                )
                .await;
                let _ = manager
                    .checkpoints
                    .update_status(&task_info.run_id, terminal);
                manager.clear_active_if(&task_key).await;
                manager.launch_next(&task_key).await;
            });
            self.active.lock().await.insert(
                key,
                ActiveRun {
                    info: queued.info,
                    handle,
                    cancel_requested,
                },
            );
            let _ = start_tx.send(());
        })
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
        // 本进程不是持有者时这一轮可能正跑在对端：把中断请求上行给持有者
        let tracked = self
            .remote_runs
            .lock()
            .await
            .iter()
            .find(|(tracked, _)| tracked == run_id)
            .map(|(_, key)| key.clone());
        let Some(key) = tracked else {
            return Ok(false);
        };
        self.abort_remote(run_id, &key).await
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
            if self.active.lock().await.contains_key(key) {
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

/// 执行 Agent 并把 RunnerEvent 写入会话事件总线。
async fn run_agent(
    paths: SaiPaths,
    request: StartRunRequest,
    info: ActiveRunInfo,
    bus: ActorHandle,
    inter_message_source: Arc<dyn InterMessageSource>,
    cancel_requested: Arc<std::sync::atomic::AtomicBool>,
) -> RunCheckpointStatus {
    let mode = match AgentMode::parse(request.mode.as_deref()) {
        Ok(mode) => mode,
        Err(error) => {
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.failed",
                json!({ "message": error.to_string(), "detail": crate::llm::error_detail_text(&error) }),
            ));
            return RunCheckpointStatus::Failed;
        }
    };
    let submission = match request.kind {
        RunKind::Conversation => {
            let mut input = UserInputSubmission::new(request.input, mode);
            input = input.with_image_urls(request.image_url.into_iter().chain(request.image_urls));
            input = input.with_turn_id(info.run_id.clone());
            RunnerSubmission::user_input(SubmissionSource::Web, input)
        }
        RunKind::Compaction => RunnerSubmission::control(
            SubmissionSource::Web,
            mode,
            ControlSubmission::new(crate::control_commands::ControlCommand::Compact),
        ),
        RunKind::GoalContinuation => RunnerSubmission::user_input(
            SubmissionSource::Web,
            UserInputSubmission::new(String::new(), mode).with_goal_continuation(),
        ),
    }
    .with_session_id(info.session_id.clone())
    .with_final_summary(true);
    // 会话级组装器由事件总线持有，这里只声明轮次边界
    let _ = bus.begin_run(&info.run_id, &info.input, &info.image_urls);
    let mut sink = |event| bus.publish(event);
    let run_config = match resolve_run_config(
        &paths,
        request.agent_id.as_deref(),
        request.provider_id.as_deref(),
        request.model.as_deref(),
        request.thinking_level.as_deref(),
    ) {
        Ok(config) => config,
        Err(error) => {
            let _ = bus.emit(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.failed",
                json!({ "message": error.to_string(), "detail": crate::llm::error_detail_text(&error) }),
            ));
            return RunCheckpointStatus::Failed;
        }
    };
    let runner = match run_config {
        Some(config) => SessionRunner::new(&paths).with_config(config),
        None => SessionRunner::new(&paths),
    }
    .with_inter_message_source(inter_message_source)
    .with_cancel_flag(cancel_requested);
    if let Err(error) = runner.run_submission(submission, &mut sink).await {
        let _ = bus.emit(WebEvent::new(
            &info.run_id,
            &info.workspace_id,
            &info.session_id,
            "run.failed",
            json!({ "message": error.to_string(), "detail": crate::llm::error_detail_text(&error) }),
        ));
        return RunCheckpointStatus::Failed;
    }
    RunCheckpointStatus::Completed
}

/// 生成工作区会话级调度键。
fn session_key(workspace_id: &str, session_id: &str) -> String {
    format!("{workspace_id}:{session_id}")
}

/// 从工作区路径推出展示名。
///
/// 观察者上行的一轮只带工作区路径（持有者要用它作为工作目录），展示名
/// 在这里补出来；路径不可用时退回空串，由调用方决定兜底。
///
/// 参数:
/// - `path`: 工作区绝对路径
///
/// 返回:
/// - 目录名
fn workspace_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
}

/// 判断指定会话是否正被一个活着的持有者进程持有。
///
/// 只看登记表：持有者心跳新鲜即判定存活，不必去查进程。读不到登记表
/// （会话目录不存在、文件损坏）时返回 false，按「没人持有」处理，
/// 由调用方走原来的恢复路径——宁可多恢复一次，也不要漏掉崩溃残留。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `session_id`: 会话标识
///
/// 返回:
/// - 是否另有存活持有者
fn session_is_held_by_a_live_process(paths: &crate::paths::SaiPaths, session_id: &str) -> bool {
    let Ok((_, state_dir)) = crate::state::locate_session_dirs(paths, session_id) else {
        return false;
    };
    crate::runner::session_holder(&state_dir)
        .as_ref()
        .is_some_and(crate::runner::holder_is_alive)
}

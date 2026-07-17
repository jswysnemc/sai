use super::assembler::EventAssembler;
use super::checkpoint::{RunCheckpoint, RunCheckpointStatus, RunCheckpointStore};
use super::model_override::resolve_run_config;
use super::{EventJournal, WebEvent};
use crate::agent::AgentMode;
use crate::paths::SaiPaths;
use crate::runner::{
    ControlSubmission, RunnerSubmission, SessionRunner, SubmissionSource, UserInputSubmission,
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

const RUN_JOURNAL_CAPACITY: usize = 32;

/// 启动一轮 Web 对话所需参数。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StartRunRequest {
    #[serde(default)]
    pub kind: RunKind,
    pub session_id: String,
    pub input: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking_level: Option<String>,
}

/// Web 运行种类。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunKind {
    #[default]
    Conversation,
    Compaction,
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
}

struct ActiveRun {
    info: ActiveRunInfo,
    handle: JoinHandle<()>,
}

#[derive(Clone)]
struct QueuedRun {
    info: ActiveRunInfo,
    workspace: WorkspaceInfo,
    request: StartRunRequest,
}

#[derive(Default)]
struct RunJournals {
    entries: HashMap<String, EventJournal>,
    order: VecDeque<String>,
}

/// 管理 Web 运行互斥、事件日志和中断句柄。
#[derive(Clone)]
pub(crate) struct RunManager {
    paths: SaiPaths,
    active: Arc<Mutex<HashMap<String, ActiveRun>>>,
    queued: Arc<Mutex<HashMap<String, VecDeque<QueuedRun>>>>,
    scheduling: Arc<Mutex<()>>,
    journals: Arc<RwLock<RunJournals>>,
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
            journals: Arc::new(RwLock::new(RunJournals::default())),
            checkpoints,
        };
        for checkpoint in manager.checkpoints.recover_running_as_interrupted()? {
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
            let journal =
                EventJournal::persistent(manager.checkpoints.event_path(&checkpoint.info.run_id));
            journal.publish(WebEvent::new(
                &checkpoint.info.run_id,
                &checkpoint.info.workspace_id,
                &checkpoint.info.session_id,
                "run.interrupted",
                json!({
                    "recovered": true,
                    "discard_user_turn": false,
                    "restore_input": null,
                }),
            ));
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
        if request.kind == RunKind::Conversation
            && request.input.trim().is_empty()
            && request.image_url.is_none()
            && request.image_urls.is_empty()
        {
            bail!("message cannot be empty");
        }
        parse_mode(request.mode.as_deref())?;
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
        };
        let journal = EventJournal::persistent(self.checkpoints.event_path(&run_id));
        self.insert_journal(run_id.clone(), journal.clone()).await;
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
            journal.publish(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.queued",
                json!({ "position": queue.len() }),
            ));
            return Ok(info);
        }
        self.spawn_run(key, queued_run, journal).await;
        Ok(info)
    }

    /// 恢复进程重启前尚未执行的排队运行。
    pub(crate) async fn resume_queued(&self) {
        let keys = self.queued.lock().await.keys().cloned().collect::<Vec<_>>();
        for key in keys {
            self.launch_next(&key).await;
        }
    }

    /// 启动已经取得会话执行权的运行。
    fn spawn_run(
        &self,
        key: String,
        queued: QueuedRun,
        journal: EventJournal,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            let (start_tx, start_rx) = oneshot::channel();
            let manager = self.clone();
            let task_info = queued.info.clone();
            let workspace_path = std::path::PathBuf::from(&queued.workspace.path);
            let paths = self.paths.clone();
            let task_key = key.clone();
            let handle = tokio::spawn(async move {
                let _ = start_rx.await;
                let terminal = crate::runtime_cwd::scope(
                    workspace_path,
                    run_agent(paths, queued.request, task_info.clone(), journal.clone()),
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
            current.handle.abort();
            let info = current.info.clone();
            drop(active);
            let _ = current.handle.await;
            self.checkpoints.update_interruption(run_id, false, None)?;
            if let Some(journal) = self.journal(run_id).await {
                journal.publish(WebEvent::new(
                    &info.run_id,
                    &info.workspace_id,
                    &info.session_id,
                    "run.interrupted",
                    json!({
                        "discard_user_turn": false,
                        "restore_input": null,
                    }),
                ));
            }
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
            let journal = self
                .journal(run_id)
                .await
                .unwrap_or_else(|| EventJournal::persistent(self.checkpoints.event_path(run_id)));
            journal.publish(WebEvent::new(
                &queued.info.run_id,
                &queued.info.workspace_id,
                &queued.info.session_id,
                "run.interrupted",
                json!({
                    "queued": true,
                    "discard_user_turn": true,
                    "restore_input": queued.info.input,
                }),
            ));
            return Ok(true);
        }
        Ok(false)
    }

    /// 返回指定运行事件日志。
    pub(crate) async fn journal(&self, run_id: &str) -> Option<EventJournal> {
        if let Some(journal) = self.journals.read().await.entries.get(run_id).cloned() {
            return Some(journal);
        }
        self.checkpoints
            .get(run_id)
            .map(|_| EventJournal::persistent(self.checkpoints.event_path(run_id)))
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

    /// 保存运行事件日志并移除最早的过期日志。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    /// - `journal`: 运行事件日志
    async fn insert_journal(&self, run_id: String, journal: EventJournal) {
        let mut journals = self.journals.write().await;
        journals.entries.insert(run_id.clone(), journal);
        journals.order.push_back(run_id);
        while journals.order.len() > RUN_JOURNAL_CAPACITY {
            if let Some(expired_id) = journals.order.pop_front() {
                journals.entries.remove(&expired_id);
            }
        }
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
            let journal = self.journal(&queued.info.run_id).await.unwrap_or_else(|| {
                EventJournal::persistent(self.checkpoints.event_path(&queued.info.run_id))
            });
            journal.publish(WebEvent::new(
                &queued.info.run_id,
                &queued.info.workspace_id,
                &queued.info.session_id,
                "run.dequeued",
                json!({}),
            ));
            self.spawn_run(key.to_string(), queued, journal).await;
        })
    }
}

/// 执行 Agent 并把 RunnerEvent 写入事件日志。
async fn run_agent(
    paths: SaiPaths,
    request: StartRunRequest,
    info: ActiveRunInfo,
    journal: EventJournal,
) -> RunCheckpointStatus {
    let mode = match parse_mode(request.mode.as_deref()) {
        Ok(mode) => mode,
        Err(error) => {
            journal.publish(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.failed",
                json!({ "message": error.to_string(), "detail": format!("{error:#}") }),
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
    }
    .with_session_id(info.session_id.clone())
    .with_final_summary(true);
    let mut assembler = EventAssembler::new(&info.run_id, &info.workspace_id, &info.session_id);
    let mut sink = |event| {
        for event in assembler.map(event) {
            journal.publish(event);
        }
        Ok(())
    };
    let run_config = match resolve_run_config(
        &paths,
        request.agent_id.as_deref(),
        request.provider_id.as_deref(),
        request.model.as_deref(),
        request.thinking_level.as_deref(),
    ) {
        Ok(config) => config,
        Err(error) => {
            journal.publish(WebEvent::new(
                &info.run_id,
                &info.workspace_id,
                &info.session_id,
                "run.failed",
                json!({ "message": error.to_string(), "detail": format!("{error:#}") }),
            ));
            return RunCheckpointStatus::Failed;
        }
    };
    let runner = match run_config {
        Some(config) => SessionRunner::new(&paths).with_config(config),
        None => SessionRunner::new(&paths),
    };
    if let Err(error) = runner.run_submission(submission, &mut sink).await {
        journal.publish(WebEvent::new(
            &info.run_id,
            &info.workspace_id,
            &info.session_id,
            "run.failed",
            json!({ "message": error.to_string(), "detail": format!("{error:#}") }),
        ));
        return RunCheckpointStatus::Failed;
    }
    RunCheckpointStatus::Completed
}

/// 生成工作区会话级调度键。
fn session_key(workspace_id: &str, session_id: &str) -> String {
    format!("{workspace_id}:{session_id}")
}

/// 解析 Web 端运行模式。
fn parse_mode(value: Option<&str>) -> Result<AgentMode> {
    match value.unwrap_or("yolo").trim().to_ascii_lowercase().as_str() {
        "plan" => Ok(AgentMode::Plan),
        "audited" | "audit" => Ok(AgentMode::Audited),
        "yolo" | "" => Ok(AgentMode::Yolo),
        value => bail!("unsupported run mode: {value}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// 创建运行管理器测试路径。
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[tokio::test]
    async fn keeps_only_recent_run_journals() {
        let temp = tempfile::tempdir().unwrap();
        let manager = RunManager::new(&test_paths(temp.path().to_path_buf())).unwrap();
        for index in 0..=RUN_JOURNAL_CAPACITY {
            manager
                .insert_journal(format!("run-{index}"), EventJournal::new())
                .await;
        }
        assert!(manager.journal("run-0").await.is_none());
        assert!(manager
            .journal(&format!("run-{RUN_JOURNAL_CAPACITY}"))
            .await
            .is_some());
    }

    /// 验证同一会话的第二次提交会进入持久化队列。
    #[tokio::test]
    async fn queues_second_submission_for_same_session() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let manager = RunManager::new(&paths).unwrap();
        let workspace = WorkspaceInfo {
            id: "workspace".to_string(),
            name: "workspace".to_string(),
            path: temp.path().display().to_string(),
            last_opened_at: String::new(),
        };
        let key = session_key(&workspace.id, "session");
        let task = tokio::spawn(std::future::pending::<()>());
        manager.active.lock().await.insert(
            key,
            ActiveRun {
                info: ActiveRunInfo {
                    run_id: "running".to_string(),
                    workspace_id: workspace.id.clone(),
                    session_id: "session".to_string(),
                    input: "first".to_string(),
                    image_urls: Vec::new(),
                    status: RunCheckpointStatus::Running,
                    discard_user_turn: false,
                    restore_input: None,
                },
                handle: task,
            },
        );

        let queued = manager
            .start(
                workspace,
                StartRunRequest {
                    kind: RunKind::Conversation,
                    session_id: "session".to_string(),
                    input: "second".to_string(),
                    agent_id: None,
                    image_url: None,
                    image_urls: Vec::new(),
                    mode: None,
                    provider_id: None,
                    model: None,
                    thinking_level: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(queued.status, RunCheckpointStatus::Queued);
        assert_eq!(
            manager.checkpoints.get(&queued.run_id).unwrap().status,
            RunCheckpointStatus::Queued
        );
    }
}

use super::*;
use crate::config::AppConfig;
use crate::goal::GoalStatus;
use crate::state::StateStore;
use crate::tools::command::poll_background_completions_in_scope;
use crate::tools::subagent_state::list_subagents_for_owner;
use std::collections::HashSet;
use std::sync::OnceLock;
use std::time::Duration;

const WAIT_INTERVAL: Duration = Duration::from_millis(500);

/// 运行结束后是否应该再排一轮目标续轮。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContinuationDecision {
    /// 目标仍活动，且没有挡路的后台工作
    Continue,
    /// 目标仍活动，但子智能体或后台命令还在运行
    Wait,
    /// 不续轮
    Stop,
}

/// 根据终态、队列和后台工作决定是否续轮。
///
/// 参数:
/// - `status`: 刚结束的运行终态
/// - `goal_active`: 会话目标是否仍为活动
/// - `queue_empty`: 该会话是否没有待执行的用户消息
/// - `running_work`: 是否仍有运行中的子智能体或后台命令
///
/// 返回:
/// - 续轮决策
pub(super) fn decide_continuation(
    status: RunCheckpointStatus,
    goal_active: bool,
    queue_empty: bool,
    running_work: bool,
) -> ContinuationDecision {
    if status != RunCheckpointStatus::Completed || !goal_active || !queue_empty {
        return ContinuationDecision::Stop;
    }
    if running_work {
        ContinuationDecision::Wait
    } else {
        ContinuationDecision::Continue
    }
}

impl RunManager {
    /// 【Web】【目标续轮】一轮成功结束后，按 TUI 的空闲条件再排一轮。
    ///
    /// 参数:
    /// - `key`: 会话调度键
    /// - `completed`: 刚结束的运行
    /// - `status`: 运行终态
    ///
    /// 返回:
    /// - 无；失败只放弃这一次续轮，不把已完成的运行改成失败
    pub(super) async fn schedule_goal_continuation(
        &self,
        key: &str,
        completed: &QueuedRun,
        status: RunCheckpointStatus,
    ) {
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let queue_empty = self
            .queued
            .lock()
            .await
            .get(key)
            .is_none_or(|queue| queue.is_empty());
        let goal_active = goal_is_active(&self.paths, &completed.workspace.path, &completed.request.session_id);
        let running_work = session_has_running_work(
            &self.paths,
            &completed.workspace.path,
            &completed.request.session_id,
        )
        .await;
        if status == RunCheckpointStatus::Completed {
            mark_unread_if_unfocused(&self.paths, &completed.workspace.path, &completed.request.session_id);
        }
        match decide_continuation(status, goal_active, queue_empty, running_work) {
            ContinuationDecision::Stop => {}
            ContinuationDecision::Wait => self.spawn_goal_waiter(key.to_string(), completed.clone()),
            ContinuationDecision::Continue => {
                self.enqueue_goal_continuation(completed).await;
            }
        }
    }

    /// 把目标续轮放进会话队列，不在这里启动。
    ///
    /// 参数:
    /// - `completed`: 刚结束的运行，用于继承工作区、模式和模型
    ///
    /// 返回:
    /// - 无
    async fn enqueue_goal_continuation(&self, completed: &QueuedRun) {
        let key = session_key(&completed.info.workspace_id, &completed.info.session_id);
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let request = StartRunRequest {
            kind: RunKind::GoalContinuation,
            session_id: completed.request.session_id.clone(),
            input: String::new(),
            agent_id: completed.request.agent_id.clone(),
            image_url: None,
            image_urls: Vec::new(),
            mode: completed.request.mode.clone(),
            provider_id: completed.request.provider_id.clone(),
            model: completed.request.model.clone(),
            thinking_level: completed.request.thinking_level.clone(),
            insert_at: QueueInsertAt::Turn,
        };
        let info = ActiveRunInfo {
            run_id: run_id.clone(),
            workspace_id: completed.info.workspace_id.clone(),
            session_id: completed.info.session_id.clone(),
            input: String::new(),
            image_urls: Vec::new(),
            status: RunCheckpointStatus::Queued,
            discard_user_turn: false,
            restore_input: None,
            insert_at: QueueInsertAt::Turn,
        };
        if self
            .checkpoints
            .upsert(RunCheckpoint {
                info: info.clone(),
                workspace: completed.workspace.clone(),
                request: request.clone(),
                status: RunCheckpointStatus::Queued,
                updated_at: String::new(),
            })
            .is_err()
        {
            return;
        }
        self.queued.lock().await.entry(key).or_default().push_back(QueuedRun {
            info,
            workspace: completed.workspace.clone(),
            request,
        });
    }

    /// 后台工作结束后再判断一次是否续轮。
    ///
    /// 参数:
    /// - `key`: 会话调度键
    /// - `completed`: 触发等待的已结束运行
    ///
    /// 返回:
    /// - 无
    fn spawn_goal_waiter(&self, key: String, completed: QueuedRun) {
        let waiters = goal_waiters();
        if !waiters.lock().unwrap().insert(key.clone()) {
            return;
        }
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(WAIT_INTERVAL).await;
                if manager.shutting_down.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let busy = manager.active.lock().await.contains_key(&key)
                    || manager
                        .queued
                        .lock()
                        .await
                        .get(&key)
                        .is_some_and(|queue| !queue.is_empty());
                if busy {
                    break;
                }
                if !goal_is_active(&manager.paths, &completed.workspace.path, &completed.request.session_id) {
                    break;
                }
                if session_has_running_work(
                    &manager.paths,
                    &completed.workspace.path,
                    &completed.request.session_id,
                )
                .await
                {
                    continue;
                }
                manager.enqueue_goal_continuation(&completed).await;
                manager.launch_next(&key).await;
                break;
            }
            goal_waiters().lock().unwrap().remove(&key);
        });
    }
}

/// 返回进程内的续轮等待集合，避免同一会话重复等待。
fn goal_waiters() -> &'static std::sync::Mutex<HashSet<String>> {
    static WAITERS: OnceLock<std::sync::Mutex<HashSet<String>>> = OnceLock::new();
    WAITERS.get_or_init(|| std::sync::Mutex::new(HashSet::new()))
}

/// 判断会话目标是否仍允许自动续轮。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
/// - `session_id`: 会话标识
///
/// 返回:
/// - 目标存在且状态为活动时返回 true
/// 未被当前工作区选中的会话，一轮结束后标为未读。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
/// - `session_id`: 刚结束的会话
///
/// 返回:
/// - 无；写入失败不影响续轮
fn mark_unread_if_unfocused(paths: &SaiPaths, workspace_path: &str, session_id: &str) {
    let Ok(current) = crate::state::active_session_id_for_workspace(paths, std::path::Path::new(workspace_path)) else {
        return;
    };
    if current == session_id {
        return;
    }
    let _ = crate::state::patch_sidebar_index(
        paths,
        crate::state::SidebarIndexPatch {
            mark_unread: Some(session_id.to_string()),
            ..crate::state::SidebarIndexPatch::default()
        },
    );
}

fn goal_is_active(paths: &SaiPaths, workspace_path: &str, session_id: &str) -> bool {
    StateStore::for_workspace_session(paths, std::path::Path::new(workspace_path), session_id)
        .ok()
        .and_then(|state| state.goal().ok().flatten())
        .is_some_and(|goal| goal.status == GoalStatus::Active)
}

/// 判断会话是否还有会挡住续轮的后台工作。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `workspace_path`: 工作区目录
/// - `session_id`: 会话标识
///
/// 返回:
/// - 有运行中的子智能体或后台命令时返回 true
async fn session_has_running_work(paths: &SaiPaths, workspace_path: &str, session_id: &str) -> bool {
    let Ok(state) = StateStore::for_workspace_session(paths, std::path::Path::new(workspace_path), session_id) else {
        return false;
    };
    let owner_key = state.state_dir().display().to_string();
    if list_subagents_for_owner(&owner_key)
        .iter()
        .any(|snapshot| snapshot.status == "running")
    {
        return true;
    }
    let Ok(config) = AppConfig::load(paths) else {
        return false;
    };
    poll_background_completions_in_scope(paths, &config, session_id, None, |_| true)
        .await
        .is_ok_and(|(_, running)| running > 0)
}

#[cfg(test)]
mod tests {
    use super::{decide_continuation, ContinuationDecision};
    use crate::web::runs::checkpoint::RunCheckpointStatus;

    /// 只有成功结束、目标仍活动且队列为空时才续轮。
    #[test]
    fn continues_only_after_completed_active_goal() {
        assert_eq!(
            decide_continuation(RunCheckpointStatus::Completed, true, true, false),
            ContinuationDecision::Continue
        );
        assert_eq!(
            decide_continuation(RunCheckpointStatus::Completed, true, true, true),
            ContinuationDecision::Wait
        );
        assert_eq!(
            decide_continuation(RunCheckpointStatus::Interrupted, true, true, false),
            ContinuationDecision::Stop
        );
        assert_eq!(
            decide_continuation(RunCheckpointStatus::Completed, false, true, false),
            ContinuationDecision::Stop
        );
        assert_eq!(
            decide_continuation(RunCheckpointStatus::Completed, true, false, false),
            ContinuationDecision::Stop
        );
    }
}

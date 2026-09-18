use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::tools::command::BackgroundCommandStore;
use crate::tools::mesh::pending_messages;
use crate::tools::subagent_state::{
    list_subagents_for_owner, subagent_message_counts, SubagentSnapshot,
};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// 【自动续聊】【打开边界】Agent 持有本次打开的有效期，销毁或切换时撤销旧监听。
pub(super) struct ExternalWakePolicy {
    scope: Arc<ExternalWakeScope>,
}

/// 【自动续聊】【回执范围】仅保存打开前已存在的标识，不修改任务、日志或确认记录。
pub(super) struct ExternalWakeScope {
    pub(super) id: u64,
    active: AtomicBool,
    background_ids: Option<HashSet<String>>,
    subagent_messages_at_open: HashMap<String, usize>,
    mesh_ids: HashSet<String>,
    previous_goal: Option<String>,
    goal_resumed: AtomicBool,
}

impl ExternalWakePolicy {
    /// 【自动续聊】【打开边界】记录本次打开之前的后台任务、子任务、信箱与目标。
    /// 参数: paths 为应用路径，state 为即将打开的会话
    /// 返回: 会话独立的自动唤醒范围；状态读取错误时返回失败
    pub(super) fn capture(paths: &SaiPaths, state: &StateStore) -> Result<Self> {
        static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
        let owner = state.state_dir().display().to_string();
        // 1. 【自动续聊】【旧任务隔离】任务表暂时不可读时不接收后台自动唤醒，避免稍后误接管旧任务
        let background_ids = BackgroundCommandStore::new(paths.state_dir.clone())
            .load()
            .ok()
            .map(|tasks| {
                tasks
                    .into_iter()
                    .filter(|task| task.owned_by_session(state.session_id()))
                    .map(|task| task.id)
                    .collect()
            });
        // 2. 【自动续聊】【旧消息隔离】同时记录已消费和仍排队的旧消息，避免延迟完成被视为新任务
        let subagent_messages_at_open = list_subagents_for_owner(&owner)
            .into_iter()
            .map(|task| {
                let boundary = if task.persistent {
                    subagent_message_counts(&task.id).0
                } else {
                    usize::MAX
                };
                (task.id, boundary)
            })
            .collect();
        Ok(Self {
            scope: Arc::new(ExternalWakeScope {
                id: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
                active: AtomicBool::new(true),
                background_ids,
                subagent_messages_at_open,
                mesh_ids: pending_messages(state.state_dir(), state.session_id())
                    .into_iter()
                    .map(|message| message.id)
                    .collect(),
                previous_goal: state.goal()?.map(|goal| goal.id),
                goal_resumed: AtomicBool::new(false),
            }),
        })
    }

    /// 取得当前有效期句柄；无参数，返回供监听器共享的范围。
    pub(super) fn scope(&self) -> Arc<ExternalWakeScope> {
        self.scope.clone()
    }

    /// 用户主动继续当前任务后允许恢复已有目标；无参数，无返回值。
    pub(super) fn resume_goal(&self) {
        self.scope.goal_resumed.store(true, Ordering::Release);
    }
}

impl Drop for ExternalWakePolicy {
    /// 【自动续聊】【监听撤销】关闭或切换会话时撤销所有旧句柄；无参数，无返回值。
    fn drop(&mut self) {
        self.scope.active.store(false, Ordering::Release);
    }
}

impl ExternalWakeScope {
    /// 判断所属会话是否仍然打开；无参数，返回有效期状态。
    pub(super) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// 判断目标是否由本次交互创建或明确恢复；参数为目标标识，返回是否允许自动续作。
    pub(super) fn allows_goal(&self, id: &str) -> bool {
        self.is_active()
            && (self.previous_goal.as_deref() != Some(id)
                || self.goal_resumed.load(Ordering::Acquire))
    }

    /// 判断旧任务是否属于用户明确恢复的目标；参数为任务目标，返回是否可以重新接收回执。
    fn goal_task_resumed(&self, goal_id: Option<&str>) -> bool {
        goal_id.is_some()
            && goal_id == self.previous_goal.as_deref()
            && self.goal_resumed.load(Ordering::Acquire)
    }

    /// 判断后台任务是否可以唤醒当前会话；参数为任务及目标标识，返回是否允许投递。
    pub(super) fn allows_background(&self, id: &str, goal_id: Option<&str>) -> bool {
        self.is_active()
            && self
                .background_ids
                .as_ref()
                .is_some_and(|ids| !ids.contains(id) || self.goal_task_resumed(goal_id))
    }

    /// 【自动续聊】【新消息授权】已有子任务消费新指令后才允许回执，单纯入队不能重发旧结果。
    /// 参数: task 为当前子任务快照
    /// 返回: 本次创建、明确恢复目标或已消费新指令时允许投递和等待
    pub(super) fn allows_subagent(&self, task: &SubagentSnapshot) -> bool {
        self.is_active()
            && (self.goal_task_resumed(task.goal_id.as_deref())
                || self
                    .subagent_messages_at_open
                    .get(&task.id)
                    .is_none_or(|boundary| {
                        task.persistent && subagent_message_counts(&task.id).1 > *boundary
                    }))
    }

    /// 判断信箱消息是否为打开后的新消息；参数为消息标识，返回是否允许自动投递。
    pub(super) fn allows_mesh(&self, id: &str) -> bool {
        self.is_active() && !self.mesh_ids.contains(id)
    }
}

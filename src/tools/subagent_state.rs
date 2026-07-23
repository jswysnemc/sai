use super::readable_tool_name;
use super::subagent_event::{SubagentEventJournal, SubagentStreamEvent};
use super::subagent_persistence::{self, PersistedSubagent};
use super::subagent_timeline::{SubagentTimeline, SubagentTimelineEntry};
use crate::i18n::is_zh;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;

mod lifecycle;
mod persistence;
mod queries;
mod record_access;
mod timeline_queries;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use lifecycle::{create_subagent, create_subagent_for_owner};
pub(crate) use lifecycle::{
    create_subagent_for_owner_goal, finish_subagent, set_subagent_worktree,
    set_subagent_worktree_merge, timeline_streaming_text, timeline_tool_finished,
    timeline_tool_started, update_subagent_progress,
};
use persistence::{
    ensure_owner_loaded, persist_owner_locked, publish_record, subagents, unix_seconds,
};
#[cfg(test)]
pub(crate) use queries::take_finished_notices;
pub(crate) use queries::{
    acknowledge_finished_notices, cancel_subagent, cancel_subagent_for_owner, list_subagents,
    list_subagents_for_owner, pending_finished_notices, subagent_snapshot,
    subagent_snapshot_for_owner,
};
use record_access::running_record;
pub(crate) use timeline_queries::{subagent_event_stream, subagent_timeline};

static SUBAGENTS: OnceLock<Mutex<HashMap<String, SubagentRecord>>> = OnceLock::new();
static LOADED_OWNERS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct SubagentSnapshot {
    pub(crate) id: String,
    /// 创建子智能体时关联的持续目标，旧记录缺失时保持为空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) goal_id: Option<String>,
    pub(crate) description: String,
    pub(crate) subagent_type: String,
    pub(crate) status: String,
    pub(crate) max_steps: usize,
    pub(crate) started_at: u64,
    pub(crate) updated_at: u64,
    pub(crate) step: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stats: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parent_workdir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) worktree_merge: Option<Value>,
}

/// 子智能体运行过程中的一次进度更新。
#[derive(Debug, Clone, Default)]
pub(crate) struct SubagentProgressUpdate {
    pub(crate) step: Option<usize>,
    pub(crate) phase: Option<String>,
    pub(crate) last_tool: Option<String>,
}

struct SubagentRecord {
    owner_key: String,
    snapshot: SubagentSnapshot,
    cancel: Option<oneshot::Sender<()>>,
    /// 完成事件是否已通知主 Agent,避免重复提醒
    finish_notified: bool,
    /// 执行时间线,供详情页实时流式渲染
    timeline: SubagentTimeline,
    event_journal: SubagentEventJournal,
}

/// 已完成但尚未通知主 Agent 的子智能体摘要。
#[derive(Debug, Clone)]
pub(crate) struct FinishedSubagentNotice {
    pub(crate) id: String,
    pub(crate) goal_id: Option<String>,
    pub(crate) description: String,
    pub(crate) status: String,
}

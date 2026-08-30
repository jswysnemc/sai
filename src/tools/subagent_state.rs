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
mod messaging;
mod persistence;
mod queries;
mod record_access;
mod timeline_queries;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use lifecycle::{create_subagent, create_subagent_for_owner};
pub(crate) use lifecycle::{
    create_subagent_for_owner_goal, finish_subagent, park_subagent, resume_subagent,
    set_subagent_worktree, set_subagent_worktree_merge, timeline_streaming_text,
    timeline_tool_finished, timeline_tool_started, update_subagent_progress,
};
pub(crate) use messaging::{
    drain_subagent_inbox, queue_subagent_mesh_message, queue_subagent_message,
    queue_subagent_message_for_owner, request_subagent_stop_for_owner, subagent_inbox_len,
    subagent_messages, subagent_stop_requested,
};
use persistence::{
    ensure_owner_loaded, persist_owner_locked, publish_record, subagents, unix_seconds,
};
#[cfg(test)]
pub(crate) use queries::take_finished_notices;
pub(crate) use queries::{
    acknowledge_finished_notices, cancel_subagent, cancel_subagent_for_owner,
    clear_subagents_for_owner, list_subagents_for_owner, pending_finished_notices,
    subagent_snapshot, subagent_snapshot_for_owner,
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
    /// 是否为持久子智能体：任务段完成后进入 idle 待命，可继续接收消息
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) persistent: bool,
    /// 队列中尚未注入对话的追加消息数量
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(crate) pending_messages: usize,
    /// 已完成的任务段数量（持久子智能体每次进入待命时加一）
    #[serde(default, skip_serializing_if = "is_zero")]
    pub(crate) turns_completed: usize,
}

/// 判断计数字段是否可以在序列化时省略。
///
/// 参数:
/// - `value`: 计数值
///
/// 返回:
/// - 为零时返回 true
fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// 子智能体运行过程中的一次进度更新。
#[derive(Debug, Clone, Default)]
pub(crate) struct SubagentProgressUpdate {
    pub(crate) step: Option<usize>,
    pub(crate) phase: Option<String>,
    pub(crate) last_tool: Option<String>,
    /// 累计用量快照；运行期每轮写入，供底部面板实时显示 token 消耗
    pub(crate) stats: Option<Value>,
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
    /// 尚未注入子智能体对话的追加消息队列（来自主代理或用户）
    inbox: Vec<SubagentInboxMessage>,
    /// 已注入对话的消息历史，供详情查询回放
    message_log: Vec<SubagentInboxMessage>,
    /// 持久子智能体的优雅结束请求；worker 在任务段间隙检查
    stop_request: Option<SubagentStopRequest>,
}

/// 投递给子智能体的一条追加消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubagentInboxMessage {
    /// 消息来源：`parent`（主代理）、`user`（用户留言）或网格地址
    pub(crate) from: String,
    /// 消息正文
    pub(crate) text: String,
    /// 入队时间（Unix 秒）
    pub(crate) queued_at: u64,
    /// 网格消息 id；非网格消息为空，旧记录反序列化为 `None`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    /// 网格回复地址；非网格消息为空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reply_to: Option<String>,
    /// 网格发送方地址；非网格消息为空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) from_addr: Option<String>,
}

/// 网格消息投递到子智能体信箱时携带的关联信息。
#[derive(Debug, Clone, Default)]
pub(crate) struct MeshMessageMeta {
    /// 网格消息 id
    pub(crate) id: Option<String>,
    /// 回复应投递到的地址
    pub(crate) reply_to: Option<String>,
    /// 发送方地址
    pub(crate) from_addr: Option<String>,
}

/// 持久子智能体的优雅结束请求。
#[derive(Debug, Clone, Copy)]
pub(crate) struct SubagentStopRequest {
    /// 结束时是否把 worktree 变更 apply 回主工作区
    pub(crate) apply: bool,
}

/// 已完成但尚未通知主 Agent 的子智能体摘要。
#[derive(Debug, Clone)]
pub(crate) struct FinishedSubagentNotice {
    pub(crate) id: String,
    pub(crate) goal_id: Option<String>,
    pub(crate) description: String,
    pub(crate) status: String,
    pub(crate) updated_at: u64,
}

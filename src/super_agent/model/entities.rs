use super::enums::{
    GroupStatus, MemberRole, MemberStatus, MessageKind, MessageSenderKind, ReportStatus,
    SubscriptionMode, TaskStatus, TriggerDisposition, WatchdogSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Super-Agent 协作群组。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: String,
    pub workspace_path: String,
    pub status: GroupStatus,
    pub leader_member_id: String,
    pub default_thread_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 群组成员（Leader 或 Worker/Runner）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub role: MemberRole,
    pub status: MemberStatus,
    pub specialty: String,
    pub subagent_type: String,
    pub model_label: String,
    pub workdir: String,
    pub worktree_branch: Option<String>,
    pub active_task_id: Option<String>,
    pub last_activity: String,
    pub last_heartbeat_at: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 群组内话题线程（对应论文 3.2 节 Thread）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupThread {
    pub id: String,
    pub group_id: String,
    pub title: String,
    pub channel: String,
    pub external_ref: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Thread 与 Worker 之间的多对多订阅关系（对应论文 3.2 节 Subscription）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSubscription {
    pub id: String,
    pub group_id: String,
    pub thread_id: String,
    pub member_id: String,
    pub mode: SubscriptionMode,
    pub created_at: i64,
}

/// 群组聊天与协作时间线消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessage {
    pub id: String,
    pub seq: i64,
    pub group_id: String,
    pub thread_id: String,
    pub sender_kind: MessageSenderKind,
    pub sender_id: String,
    pub sender_name: String,
    pub recipient_id: Option<String>,
    pub recipient_name: Option<String>,
    pub kind: MessageKind,
    pub channel_source: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub related_task_id: Option<String>,
    pub related_report_id: Option<String>,
    pub created_at: i64,
}

/// 环境触发事件（对应论文第 3 节 Trigger）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerEvent {
    pub id: String,
    pub group_id: String,
    pub thread_id: String,
    pub source_channel: String,
    pub actor: String,
    pub content: String,
    pub disposition: TriggerDisposition,
    pub route_path: String,
    pub reason: String,
    pub target_member_ids: Vec<String>,
    pub metadata: Value,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Leader 编排并分配给 Worker(Runner) 的任务工单。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuperTask {
    pub id: String,
    pub group_id: String,
    pub thread_id: String,
    pub title: String,
    pub objective: String,
    pub assigned_member_id: Option<String>,
    pub assigned_member_name: Option<String>,
    pub origin_channel: String,
    pub origin_target: Value,
    pub status: TaskStatus,
    pub latest_report_status: Option<ReportStatus>,
    pub latest_report_summary: Option<String>,
    pub deliverable: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Worker(Runner) 提交给 Leader 的结构化成果报告。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerReport {
    pub id: String,
    pub group_id: String,
    pub thread_id: String,
    pub task_id: Option<String>,
    pub worker_member_id: String,
    pub worker_name: String,
    pub status: ReportStatus,
    pub summary: String,
    pub details: String,
    pub artifacts: Vec<String>,
    pub needs_human_question: Option<String>,
    pub synthesized_by_leader: bool,
    pub created_at: i64,
}

/// 群组与外部消息渠道的绑定关系。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelBinding {
    pub id: String,
    pub group_id: String,
    pub channel: String,
    pub external_chat_id: String,
    pub label: String,
    pub admission_mode: String,
    pub enabled: bool,
    pub last_inbound_at: Option<i64>,
    pub last_outbound_at: Option<i64>,
    pub created_at: i64,
}

/// Leader 向渠道主动递交成果的回执记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDeliveryReceipt {
    pub id: String,
    pub group_id: String,
    pub task_id: Option<String>,
    pub channel: String,
    pub target: String,
    pub status: String,
    pub preview: String,
    pub error: Option<String>,
    pub created_at: i64,
}

/// Watchdog 巡检与自愈事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchdogEvent {
    pub id: String,
    pub group_id: Option<String>,
    pub severity: WatchdogSeverity,
    pub kind: String,
    pub summary: String,
    pub details: String,
    pub resolved: bool,
    pub created_at: i64,
}

/// 群组列表摘要项（供 TUI 侧栏与 WebUI 左侧栏展示）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSummary {
    pub group: Group,
    pub leader_status: MemberStatus,
    pub worker_count: usize,
    pub running_worker_count: usize,
    pub open_task_count: usize,
    pub pending_review_count: usize,
    pub message_count: usize,
    pub bound_channels: Vec<String>,
    pub last_message_preview: Option<String>,
    pub last_message_at: Option<i64>,
}

/// 单个群组的完整快照视图。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDetailSnapshot {
    pub group: Group,
    pub members: Vec<GroupMember>,
    pub threads: Vec<GroupThread>,
    pub subscriptions: Vec<ThreadSubscription>,
    pub messages: Vec<GroupMessage>,
    pub tasks: Vec<SuperTask>,
    pub reports: Vec<RunnerReport>,
    pub triggers: Vec<TriggerEvent>,
    pub channels: Vec<ChannelBinding>,
    pub deliveries: Vec<ChannelDeliveryReceipt>,
}

/// 全局 Super-Agent 监控概览（对应论文 005-panel 三问原则）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuperAgentOverview {
    pub group_count: usize,
    pub running_workers: usize,
    pub total_workers: usize,
    pub active_tasks: usize,
    pub pending_synthesis_reports: usize,
    pub delivered_tasks_24h: usize,
    pub triggers_24h: usize,
    pub fast_path_triggers_24h: usize,
    pub slow_path_triggers_24h: usize,
    pub watchdog_alerts: usize,
    pub bound_channel_count: usize,
    pub groups: Vec<GroupSummary>,
    pub recent_watchdog: Vec<WatchdogEvent>,
}

/// 实时推送到 WebUI SSE 与 TUI 的增量事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuperAgentEvent {
    pub seq: u64,
    pub kind: String,
    pub group_id: Option<String>,
    pub payload: Value,
    pub timestamp: i64,
}

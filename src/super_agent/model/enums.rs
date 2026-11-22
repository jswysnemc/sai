use serde::{Deserialize, Serialize};

/// 群组状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupStatus {
    Active,
    Paused,
    Archived,
}

impl GroupStatus {
    /// 返回持久化字符串标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Archived => "archived",
        }
    }

    /// 从持久化字符串还原状态。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "paused" => Self::Paused,
            "archived" => Self::Archived,
            _ => Self::Active,
        }
    }
}

/// 群组成员角色：对应双层架构中的 Leader(Router) 与 Worker(Runner)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Leader,
    Worker,
}

impl MemberRole {
    /// 返回角色字符串标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Leader => "leader",
            Self::Worker => "worker",
        }
    }

    /// 从文本解析成员角色。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "leader" => Self::Leader,
            _ => Self::Worker,
        }
    }
}

/// 群组成员运行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatus {
    Idle,
    Running,
    Waiting,
    Blocked,
    Completed,
    Failed,
    Stopped,
}

impl MemberStatus {
    /// 返回状态字符串标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Waiting => "waiting",
            Self::Blocked => "blocked",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    /// 从文本解析成员状态。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "running" => Self::Running,
            "waiting" => Self::Waiting,
            "blocked" => Self::Blocked,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "stopped" => Self::Stopped,
            _ => Self::Idle,
        }
    }

    /// 判断成员当前是否仍处于可接收投递的活跃状态。
    pub fn is_live(self) -> bool {
        matches!(
            self,
            Self::Idle | Self::Running | Self::Waiting | Self::Blocked
        )
    }
}

/// 消息发送方类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageSenderKind {
    User,
    Leader,
    Worker,
    Watchdog,
    System,
}

impl MessageSenderKind {
    /// 返回发送方类型文本。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Leader => "leader",
            Self::Worker => "worker",
            Self::Watchdog => "watchdog",
            Self::System => "system",
        }
    }

    /// 从文本解析发送方类型。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "user" => Self::User,
            "leader" => Self::Leader,
            "worker" => Self::Worker,
            "watchdog" => Self::Watchdog,
            _ => Self::System,
        }
    }
}

/// 群组消息类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Chat,
    Dispatch,
    Report,
    Delivery,
    Progress,
    Alert,
    ChannelIn,
    ChannelOut,
}

impl MessageKind {
    /// 返回消息类型字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Dispatch => "dispatch",
            Self::Report => "report",
            Self::Delivery => "delivery",
            Self::Progress => "progress",
            Self::Alert => "alert",
            Self::ChannelIn => "channel_in",
            Self::ChannelOut => "channel_out",
        }
    }

    /// 从文本解析消息类型。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "dispatch" => Self::Dispatch,
            "report" => Self::Report,
            "delivery" => Self::Delivery,
            "progress" => Self::Progress,
            "alert" => Self::Alert,
            "channel_in" => Self::ChannelIn,
            "channel_out" => Self::ChannelOut,
            _ => Self::Chat,
        }
    }
}

/// 订阅模式：owner 接收直通事件并负责执行，context 仅接收上下文通知。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionMode {
    Owner,
    Context,
}

impl SubscriptionMode {
    /// 返回订阅模式字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Context => "context",
        }
    }

    /// 从文本解析订阅模式。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "context" => Self::Context,
            _ => Self::Owner,
        }
    }
}

/// 触发器去向（对应论文 3.1/3.2 节 Fast Path 与 Slow Path 路由记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerDisposition {
    Queued,
    FannedOut,
    Batched,
    Evaluated,
    Ignored,
    Delivered,
}

impl TriggerDisposition {
    /// 返回触发器去向文本。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::FannedOut => "fanned_out",
            Self::Batched => "batched",
            Self::Evaluated => "evaluated",
            Self::Ignored => "ignored",
            Self::Delivered => "delivered",
        }
    }

    /// 从文本解析触发器去向。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "fanned_out" => Self::FannedOut,
            "batched" => Self::Batched,
            "evaluated" => Self::Evaluated,
            "ignored" => Self::Ignored,
            "delivered" => Self::Delivered,
            _ => Self::Queued,
        }
    }
}

/// 编排任务状态（对应 Ticket 生命周期）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    WaitingReview,
    Delivered,
    Blocked,
    Failed,
    Stopped,
}

impl TaskStatus {
    /// 返回任务状态文本。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::WaitingReview => "waiting_review",
            Self::Delivered => "delivered",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    /// 从文本解析任务状态。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "running" => Self::Running,
            "waiting_review" => Self::WaitingReview,
            "delivered" => Self::Delivered,
            "blocked" => Self::Blocked,
            "failed" => Self::Failed,
            "stopped" => Self::Stopped,
            _ => Self::Pending,
        }
    }
}

/// Runner 提交给 Leader 的结构化报告状态（对齐论文与 dsh-superagent 报告模型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    Progress,
    Waiting,
    Ready,
    Blocked,
    Failed,
    Dropped,
}

impl ReportStatus {
    /// 返回报告状态字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Progress => "progress",
            Self::Waiting => "waiting",
            Self::Ready => "ready",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
            Self::Dropped => "dropped",
        }
    }

    /// 从文本解析报告状态。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "waiting" => Self::Waiting,
            "ready" => Self::Ready,
            "blocked" => Self::Blocked,
            "failed" => Self::Failed,
            "dropped" => Self::Dropped,
            _ => Self::Progress,
        }
    }

    /// 判断该报告是否要求 Leader 立即归纳或通知用户。
    pub fn requires_leader_synthesis(self) -> bool {
        matches!(
            self,
            Self::Ready | Self::Blocked | Self::Failed | Self::Waiting
        )
    }
}

/// Watchdog 事件级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchdogSeverity {
    Healthy,
    Notice,
    ActionRequired,
    Recovered,
}

impl WatchdogSeverity {
    /// 返回级别文本。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Notice => "notice",
            Self::ActionRequired => "action_required",
            Self::Recovered => "recovered",
        }
    }

    /// 从文本解析 Watchdog 级别。
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "notice" => Self::Notice,
            "action_required" => Self::ActionRequired,
            "recovered" => Self::Recovered,
            _ => Self::Healthy,
        }
    }
}

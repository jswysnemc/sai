/// 运行时资源 owner 类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OwnerKind {
    Session,
    CommandMode,
    Gateway,
    Subagent,
    RemoteControl,
}

impl OwnerKind {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::CommandMode => "command_mode",
            Self::Gateway => "gateway",
            Self::Subagent => "subagent",
            Self::RemoteControl => "remote_control",
        }
    }

    /// 从数据库状态文本恢复 owner 类型。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - owner 类型
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "command_mode" => Self::CommandMode,
            "gateway" => Self::Gateway,
            "subagent" => Self::Subagent,
            "remote_control" => Self::RemoteControl,
            _ => Self::Session,
        }
    }
}

/// 运行时进程类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProcessKind {
    BackgroundCommand,
    Gateway,
    Subagent,
    FutureProcessSpawn,
}

impl ProcessKind {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::BackgroundCommand => "background_command",
            Self::Gateway => "gateway",
            Self::Subagent => "subagent",
            Self::FutureProcessSpawn => "future_process_spawn",
        }
    }

    /// 从数据库状态文本恢复进程类型。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 进程类型
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "gateway" => Self::Gateway,
            "subagent" | "subagent_task" => Self::Subagent,
            "future_process_spawn" => Self::FutureProcessSpawn,
            _ => Self::BackgroundCommand,
        }
    }
}

/// 运行时进程状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeProcessStatus {
    Running,
    Exited,
    Stopped,
    Detached,
    Stale,
    Failed,
}

impl RuntimeProcessStatus {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Stopped => "stopped",
            Self::Detached => "detached",
            Self::Stale => "stale",
            Self::Failed => "failed",
        }
    }

    /// 从数据库状态文本恢复进程状态。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 进程状态
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "exited" => Self::Exited,
            "stopped" => Self::Stopped,
            "detached" => Self::Detached,
            "stale" => Self::Stale,
            "failed" => Self::Failed,
            _ => Self::Running,
        }
    }
}

/// 运行时恢复类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeRecoveryKind {
    SequenceGap,
    StaleOwner,
    OutputCapReached,
    DisconnectCleanupFailed,
    RemoteControlAuthFailed,
    TransportClosed,
    TransportReplayUnavailable,
}

impl RuntimeRecoveryKind {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::SequenceGap => "sequence_gap",
            Self::StaleOwner => "stale_owner",
            Self::OutputCapReached => "output_cap_reached",
            Self::DisconnectCleanupFailed => "disconnect_cleanup_failed",
            Self::RemoteControlAuthFailed => "remote_control_auth_failed",
            Self::TransportClosed => "transport_closed",
            Self::TransportReplayUnavailable => "transport_replay_unavailable",
        }
    }

    /// 从数据库状态文本恢复恢复类型。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 恢复类型
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "stale_owner" => Self::StaleOwner,
            "output_cap_reached" => Self::OutputCapReached,
            "disconnect_cleanup_failed" => Self::DisconnectCleanupFailed,
            "remote_control_auth_failed" => Self::RemoteControlAuthFailed,
            "transport_closed" => Self::TransportClosed,
            "transport_replay_unavailable" => Self::TransportReplayUnavailable,
            _ => Self::SequenceGap,
        }
    }
}

/// 运行时恢复状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeRecoveryStatus {
    Observed,
    Resolved,
    Terminal,
}

impl RuntimeRecoveryStatus {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Resolved => "resolved",
            Self::Terminal => "terminal",
        }
    }

    /// 从数据库状态文本恢复恢复状态。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 恢复状态
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "resolved" => Self::Resolved,
            "terminal" => Self::Terminal,
            _ => Self::Observed,
        }
    }
}

/// 待写入运行时进程记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct NewRuntimeProcessRecord {
    pub id: String,
    pub session_id: String,
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub process_kind: ProcessKind,
    pub command: String,
    pub cwd: String,
    pub pid: Option<i64>,
    pub pgid: Option<i64>,
    pub status: RuntimeProcessStatus,
    pub last_seq: i64,
}

/// 待写入运行时进程事件。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct NewRuntimeProcessEventRecord {
    pub process_id: String,
    pub seq: i64,
    pub stream: String,
    pub event_kind: String,
    pub payload_ref: Option<String>,
    pub payload_preview: String,
}

/// 待追加到下一序号的运行时进程事件。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct NewRuntimeProcessEventInput {
    pub process_id: String,
    pub stream: String,
    pub event_kind: String,
    pub payload_ref: Option<String>,
    pub payload_preview: String,
}

/// 待写入运行时恢复记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct NewRuntimeRecoveryRecord {
    pub session_id: String,
    pub process_id: Option<String>,
    pub kind: RuntimeRecoveryKind,
    pub status: RuntimeRecoveryStatus,
    pub reason: String,
    pub last_safe_seq: Option<i64>,
}

/// 运行时进程记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RuntimeProcessRecord {
    pub id: String,
    pub session_id: String,
    pub owner_kind: OwnerKind,
    pub owner_id: String,
    pub process_kind: ProcessKind,
    pub command: String,
    pub cwd: String,
    pub pid: Option<i64>,
    pub pgid: Option<i64>,
    pub status: RuntimeProcessStatus,
    pub last_seq: i64,
    pub last_seen_at: Option<String>,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
}

/// 运行时进程事件记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RuntimeProcessEventRecord {
    pub id: String,
    pub process_id: String,
    pub seq: i64,
    pub stream: String,
    pub event_kind: String,
    pub payload_ref: Option<String>,
    pub payload_preview: String,
    pub created_at: String,
}

/// 运行时恢复记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RuntimeRecoveryRecord {
    pub id: String,
    pub session_id: String,
    pub process_id: Option<String>,
    pub kind: RuntimeRecoveryKind,
    pub status: RuntimeRecoveryStatus,
    pub reason: String,
    pub last_safe_seq: Option<i64>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_round_trips_use_stable_database_values() {
        assert_eq!(
            OwnerKind::from_str(OwnerKind::Gateway.as_str()),
            OwnerKind::Gateway
        );
        assert_eq!(
            ProcessKind::from_str(ProcessKind::Subagent.as_str()),
            ProcessKind::Subagent
        );
        assert_eq!(
            RuntimeProcessStatus::from_str(RuntimeProcessStatus::Detached.as_str()),
            RuntimeProcessStatus::Detached
        );
        assert_eq!(
            RuntimeRecoveryKind::from_str(RuntimeRecoveryKind::OutputCapReached.as_str()),
            RuntimeRecoveryKind::OutputCapReached
        );
        assert_eq!(
            RuntimeRecoveryKind::from_str(RuntimeRecoveryKind::TransportClosed.as_str()),
            RuntimeRecoveryKind::TransportClosed
        );
        assert_eq!(
            RuntimeRecoveryKind::from_str(RuntimeRecoveryKind::TransportReplayUnavailable.as_str()),
            RuntimeRecoveryKind::TransportReplayUnavailable
        );
        assert_eq!(
            RuntimeRecoveryStatus::from_str(RuntimeRecoveryStatus::Terminal.as_str()),
            RuntimeRecoveryStatus::Terminal
        );
    }
}

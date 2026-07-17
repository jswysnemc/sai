/// Runtime transport 类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeTransportKind {
    Gateway,
    RemoteControl,
}

impl RuntimeTransportKind {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Gateway => "gateway",
            Self::RemoteControl => "remote_control",
        }
    }

    /// 从数据库状态文本恢复 Runtime transport 类型。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - Runtime transport 类型
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "remote_control" => Self::RemoteControl,
            _ => Self::Gateway,
        }
    }
}

/// 待写入 Runtime transport 状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeTransportStateUpsert {
    pub session_id: String,
    pub transport_kind: RuntimeTransportKind,
    pub transport_id: String,
    pub cursor_seq: i64,
    pub acked_seq: i64,
    pub bounded_replay_limit: i64,
    pub last_close_reason: Option<String>,
}

/// Runtime transport 持久状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeTransportState {
    pub session_id: String,
    pub transport_kind: RuntimeTransportKind,
    pub transport_id: String,
    pub cursor_seq: i64,
    pub acked_seq: i64,
    pub bounded_replay_limit: i64,
    pub last_close_reason: Option<String>,
    pub last_closed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Runtime transport 持久事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeTransportEvent {
    pub session_id: String,
    pub transport_kind: RuntimeTransportKind,
    pub transport_id: String,
    pub sequence: i64,
    pub payload_json: String,
    pub created_at: String,
}

/// Runtime transport replay 应用决策。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeTransportReplayDecision {
    Apply {
        sequence: i64,
    },
    ReplayBuffered {
        sequence: i64,
        replay_start: i64,
        replay_end: i64,
        acked_seq: i64,
    },
    SkipStale {
        sequence: i64,
        acked_seq: i64,
    },
    GapUnavailable {
        sequence: i64,
        missing_start: i64,
        missing_end: i64,
        acked_seq: i64,
    },
}

impl RuntimeTransportReplayDecision {
    /// 判断当前 replay 事件是否应该交给上层业务处理。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否应该处理事件
    #[allow(dead_code)]
    pub(crate) fn should_apply(&self) -> bool {
        matches!(self, Self::Apply { .. } | Self::ReplayBuffered { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_kind_round_trips_database_values() {
        assert_eq!(
            RuntimeTransportKind::from_str(RuntimeTransportKind::Gateway.as_str()),
            RuntimeTransportKind::Gateway
        );
        assert_eq!(
            RuntimeTransportKind::from_str(RuntimeTransportKind::RemoteControl.as_str()),
            RuntimeTransportKind::RemoteControl
        );
    }
}

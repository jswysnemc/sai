use super::model::SessionMemory;
use chrono::{DateTime, Utc};

/// 命令摘要使用的会话工作记忆投影。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMemorySummary {
    pub last_summarized_turn_id: Option<String>,
    pub last_summarized_seq: i64,
    pub checkpoint_id: Option<String>,
    pub source_turn_count: usize,
    pub token_estimate: usize,
    pub consecutive_failures: usize,
    pub is_disabled: bool,
    pub last_error: Option<String>,
    pub updated_at: String,
}

/// 构造会话工作记忆摘要投影。
///
/// 参数:
/// - `memory`: 持久化的会话工作记忆
///
/// 返回:
/// - 可放入会话快照的工作记忆摘要
pub(crate) fn summarize_memory(memory: SessionMemory) -> SessionMemorySummary {
    let is_disabled = memory_is_disabled(memory.disabled_until.as_deref());
    SessionMemorySummary {
        last_summarized_turn_id: memory.last_summarized_turn_id,
        last_summarized_seq: memory.last_summarized_seq,
        checkpoint_id: memory.checkpoint_id,
        source_turn_count: memory.source_turn_count,
        token_estimate: memory.token_estimate,
        consecutive_failures: memory.consecutive_failures,
        is_disabled,
        last_error: memory.last_error,
        updated_at: memory.updated_at,
    }
}

/// 判断会话工作记忆熔断是否仍然有效。
///
/// 参数:
/// - `disabled_until`: 熔断结束时间
///
/// 返回:
/// - 是否仍处于熔断期
fn memory_is_disabled(disabled_until: Option<&str>) -> bool {
    let Some(disabled_until) = disabled_until else {
        return false;
    };
    DateTime::parse_from_rfc3339(disabled_until)
        .map(|until| until.with_timezone(&Utc) > Utc::now())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_memory_record_for_snapshot() {
        let summary = summarize_memory(SessionMemory {
            session_id: "default".to_string(),
            summary: "current task".to_string(),
            last_summarized_turn_id: Some("turn_3".to_string()),
            last_summarized_seq: 3,
            checkpoint_id: Some("checkpoint_1".to_string()),
            source_turn_count: 3,
            token_estimate: 128,
            consecutive_failures: 2,
            disabled_until: Some("2999-01-01T00:00:00Z".to_string()),
            last_error: Some("temporary failure".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:01Z".to_string(),
        });

        assert_eq!(summary.last_summarized_seq, 3);
        assert_eq!(summary.checkpoint_id.as_deref(), Some("checkpoint_1"));
        assert_eq!(summary.source_turn_count, 3);
        assert_eq!(summary.token_estimate, 128);
        assert_eq!(summary.consecutive_failures, 2);
        assert!(summary.is_disabled);
        assert_eq!(summary.last_error.as_deref(), Some("temporary failure"));
    }

    #[test]
    fn expired_disabled_until_is_not_disabled_in_snapshot() {
        let summary = summarize_memory(SessionMemory {
            session_id: "default".to_string(),
            summary: "current task".to_string(),
            last_summarized_turn_id: Some("turn_3".to_string()),
            last_summarized_seq: 3,
            checkpoint_id: Some("checkpoint_1".to_string()),
            source_turn_count: 3,
            token_estimate: 128,
            consecutive_failures: 3,
            disabled_until: Some("2000-01-01T00:00:00Z".to_string()),
            last_error: Some("temporary failure".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:01Z".to_string(),
        });

        assert!(!summary.is_disabled);
        assert_eq!(summary.consecutive_failures, 3);
        assert_eq!(summary.last_error.as_deref(), Some("temporary failure"));
    }

    #[test]
    fn invalid_disabled_until_is_treated_as_disabled() {
        let summary = summarize_memory(SessionMemory {
            session_id: "default".to_string(),
            summary: "current task".to_string(),
            last_summarized_turn_id: Some("turn_3".to_string()),
            last_summarized_seq: 3,
            checkpoint_id: Some("checkpoint_1".to_string()),
            source_turn_count: 3,
            token_estimate: 128,
            consecutive_failures: 3,
            disabled_until: Some("invalid timestamp".to_string()),
            last_error: Some("temporary failure".to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:01Z".to_string(),
        });

        assert!(summary.is_disabled);
    }
}

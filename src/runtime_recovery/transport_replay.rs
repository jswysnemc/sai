use super::model::{NewRuntimeRecoveryRecord, RuntimeRecoveryKind, RuntimeRecoveryStatus};
use super::repository::record_recovery;
use super::transport::{advance_gateway_transport_cursor, load_gateway_transport_state};
use super::transport_event::has_gateway_transport_event_range;
use super::transport_model::RuntimeTransportReplayDecision;
use crate::state::ConversationDb;
use anyhow::Result;

/// 开始应用一条网关 transport replay 事件。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `sequence`: transport 事件序号
///
/// 返回:
/// - replay 应用决策
pub(crate) fn begin_gateway_transport_replay_event(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    sequence: u64,
) -> Result<RuntimeTransportReplayDecision> {
    let sequence = sequence_to_i64(sequence);
    let state = load_gateway_transport_state(db, session_id, gateway_id)?;
    if state.is_none() {
        advance_gateway_transport_cursor(db, session_id, gateway_id, Some(sequence as u64), None)?;
        return Ok(RuntimeTransportReplayDecision::Apply { sequence });
    }
    let acked_seq = state
        .as_ref()
        .map(|state| state.acked_seq)
        .unwrap_or_default();
    if sequence <= acked_seq {
        return Ok(RuntimeTransportReplayDecision::SkipStale {
            sequence,
            acked_seq,
        });
    }
    let expected_next = acked_seq.saturating_add(1);
    if sequence > expected_next {
        if has_gateway_transport_event_range(db, session_id, gateway_id, expected_next, sequence)? {
            advance_gateway_transport_cursor(
                db,
                session_id,
                gateway_id,
                Some(sequence as u64),
                None,
            )?;
            return Ok(RuntimeTransportReplayDecision::ReplayBuffered {
                sequence,
                replay_start: expected_next,
                replay_end: sequence,
                acked_seq,
            });
        }
        advance_gateway_transport_cursor(db, session_id, gateway_id, Some(sequence as u64), None)?;
        record_replay_unavailable(
            db,
            session_id,
            gateway_id,
            sequence,
            expected_next,
            acked_seq,
        )?;
        return Ok(RuntimeTransportReplayDecision::GapUnavailable {
            sequence,
            missing_start: expected_next,
            missing_end: sequence.saturating_sub(1),
            acked_seq,
        });
    }
    advance_gateway_transport_cursor(db, session_id, gateway_id, Some(sequence as u64), None)?;
    Ok(RuntimeTransportReplayDecision::Apply { sequence })
}

/// 写入 transport replay 不可恢复记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `sequence`: 当前收到的序号
/// - `missing_start`: 缺失起始序号
/// - `acked_seq`: 已确认序号
///
/// 返回:
/// - 写入是否成功
fn record_replay_unavailable(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    sequence: i64,
    missing_start: i64,
    acked_seq: i64,
) -> Result<()> {
    let missing_end = sequence.saturating_sub(1);
    let reason = format!(
        "gateway_id={gateway_id}; transport replay unavailable: missing seq {missing_start}..{missing_end}; cursor_seq={sequence}; acked_seq={acked_seq}"
    );
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: None,
            kind: RuntimeRecoveryKind::TransportReplayUnavailable,
            status: RuntimeRecoveryStatus::Terminal,
            reason,
            last_safe_seq: Some(acked_seq),
        },
    )?;
    Ok(())
}

/// 将 transport 序号转换为 SQLite 安全整数。
///
/// 参数:
/// - `sequence`: 原始 transport 序号
///
/// 返回:
/// - SQLite i64 序号
fn sequence_to_i64(sequence: u64) -> i64 {
    sequence.min(i64::MAX as u64) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_recovery::{
        advance_gateway_transport_cursor, load_gateway_transport_state, session_summary,
        RuntimeRecoveryKind,
    };

    #[test]
    fn replay_applies_next_unacked_sequence_and_advances_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        advance_gateway_transport_cursor(&db, "default", "qq", Some(7), Some(7)).unwrap();

        let decision = begin_gateway_transport_replay_event(&db, "default", "qq", 8).unwrap();

        assert_eq!(
            decision,
            RuntimeTransportReplayDecision::Apply { sequence: 8 }
        );
        assert!(decision.should_apply());
        let state = load_gateway_transport_state(&db, "default", "qq")
            .unwrap()
            .unwrap();
        assert_eq!(state.cursor_seq, 8);
        assert_eq!(state.acked_seq, 7);
    }

    #[test]
    fn replay_accepts_first_observed_sequence_as_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        let decision = begin_gateway_transport_replay_event(&db, "default", "qq", 42).unwrap();

        assert_eq!(
            decision,
            RuntimeTransportReplayDecision::Apply { sequence: 42 }
        );
        let state = load_gateway_transport_state(&db, "default", "qq")
            .unwrap()
            .unwrap();
        assert_eq!(state.cursor_seq, 42);
        assert_eq!(state.acked_seq, 0);
    }

    #[test]
    fn replay_skips_stale_sequence_without_moving_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        advance_gateway_transport_cursor(&db, "default", "qq", Some(9), Some(9)).unwrap();

        let decision = begin_gateway_transport_replay_event(&db, "default", "qq", 8).unwrap();

        assert_eq!(
            decision,
            RuntimeTransportReplayDecision::SkipStale {
                sequence: 8,
                acked_seq: 9,
            }
        );
        assert!(!decision.should_apply());
        let state = load_gateway_transport_state(&db, "default", "qq")
            .unwrap()
            .unwrap();
        assert_eq!(state.cursor_seq, 9);
        assert_eq!(state.acked_seq, 9);
    }

    #[test]
    fn replay_records_gap_unavailable_without_acknowledging_current_sequence() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        advance_gateway_transport_cursor(&db, "default", "qq", Some(6), Some(6)).unwrap();

        let decision = begin_gateway_transport_replay_event(&db, "default", "qq", 9).unwrap();

        assert_eq!(
            decision,
            RuntimeTransportReplayDecision::GapUnavailable {
                sequence: 9,
                missing_start: 7,
                missing_end: 8,
                acked_seq: 6,
            }
        );
        assert!(!decision.should_apply());
        let state = load_gateway_transport_state(&db, "default", "qq")
            .unwrap()
            .unwrap();
        assert_eq!(state.cursor_seq, 9);
        assert_eq!(state.acked_seq, 6);
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();
        assert_eq!(
            failure.kind,
            RuntimeRecoveryKind::TransportReplayUnavailable
        );
        assert_eq!(failure.last_safe_seq, Some(6));
        assert!(failure.reason.contains("missing seq 7..8"));
    }

    #[test]
    fn replay_uses_buffered_transport_events_when_gap_is_locally_available() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        advance_gateway_transport_cursor(&db, "default", "qq", Some(6), Some(6)).unwrap();
        crate::runtime_recovery::record_gateway_transport_event(
            &db,
            "default",
            "qq",
            7,
            &serde_json::json!({"s": 7}),
        )
        .unwrap();
        crate::runtime_recovery::record_gateway_transport_event(
            &db,
            "default",
            "qq",
            8,
            &serde_json::json!({"s": 8}),
        )
        .unwrap();
        crate::runtime_recovery::record_gateway_transport_event(
            &db,
            "default",
            "qq",
            9,
            &serde_json::json!({"s": 9}),
        )
        .unwrap();

        let decision = begin_gateway_transport_replay_event(&db, "default", "qq", 9).unwrap();

        assert_eq!(
            decision,
            RuntimeTransportReplayDecision::ReplayBuffered {
                sequence: 9,
                replay_start: 7,
                replay_end: 9,
                acked_seq: 6,
            }
        );
        assert!(decision.should_apply());
        let state = load_gateway_transport_state(&db, "default", "qq")
            .unwrap()
            .unwrap();
        assert_eq!(state.cursor_seq, 9);
        assert_eq!(state.acked_seq, 6);
        let summary = session_summary(&db, "default").unwrap();
        assert!(summary.latest_failure.is_none());
    }
}

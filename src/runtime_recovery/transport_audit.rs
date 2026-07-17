use super::model::{NewRuntimeRecoveryRecord, RuntimeRecoveryKind, RuntimeRecoveryStatus};
use super::repository::record_recovery;
use super::transport::load_gateway_transport_state;
use crate::state::ConversationDb;
use anyhow::Result;

/// 审计网关 transport 是否存在无法 replay 的未确认区间。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
///
/// 返回:
/// - 是否写入恢复记录
pub(crate) fn audit_gateway_transport_replay(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
) -> Result<bool> {
    let Some(state) = load_gateway_transport_state(db, session_id, gateway_id)? else {
        return Ok(false);
    };
    if state.cursor_seq <= state.acked_seq {
        return Ok(false);
    }
    let missing_start = state.acked_seq.saturating_add(1);
    let reason = format!(
        "gateway_id={gateway_id}; transport replay unavailable: missing seq {missing_start}..{}; cursor_seq={}; acked_seq={}",
        state.cursor_seq, state.cursor_seq, state.acked_seq
    );
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: None,
            kind: RuntimeRecoveryKind::TransportReplayUnavailable,
            status: RuntimeRecoveryStatus::Terminal,
            reason,
            last_safe_seq: Some(state.acked_seq),
        },
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_recovery::{
        advance_gateway_transport_cursor, session_summary, RuntimeRecoveryKind,
    };

    #[test]
    fn audit_records_unacked_transport_gap_as_replay_unavailable() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        advance_gateway_transport_cursor(&db, "default", "qq", Some(9), Some(6)).unwrap();

        assert!(audit_gateway_transport_replay(&db, "default", "qq").unwrap());
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();
        assert_eq!(
            failure.kind,
            RuntimeRecoveryKind::TransportReplayUnavailable
        );
        assert_eq!(failure.status, RuntimeRecoveryStatus::Terminal);
        assert_eq!(failure.last_safe_seq, Some(6));
        assert!(failure.reason.contains("missing seq 7..9"));
    }

    #[test]
    fn audit_skips_when_transport_has_no_unacked_gap() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        advance_gateway_transport_cursor(&db, "default", "qq", Some(9), Some(9)).unwrap();

        assert!(!audit_gateway_transport_replay(&db, "default", "qq").unwrap());
        let summary = session_summary(&db, "default").unwrap();
        assert!(summary.latest_failure.is_none());
    }
}

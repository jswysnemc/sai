use super::model::{NewRuntimeRecoveryRecord, RuntimeRecoveryKind, RuntimeRecoveryStatus};
use super::repository::record_recovery;
use super::transport_model::{
    RuntimeTransportKind, RuntimeTransportState, RuntimeTransportStateUpsert,
};
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Row};

const MAX_TRANSPORT_REASON_CHARS: usize = 512;
const DEFAULT_BOUNDED_REPLAY_LIMIT: i64 = 100;

/// 推进网关 transport cursor 和 ack。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `cursor_seq`: 可选已接收序号
/// - `acked_seq`: 可选已处理序号
///
/// 返回:
/// - 推进后的 transport 状态
pub(crate) fn advance_gateway_transport_cursor(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    cursor_seq: Option<u64>,
    acked_seq: Option<u64>,
) -> Result<RuntimeTransportState> {
    advance_transport_state(
        db,
        RuntimeTransportStateUpsert {
            session_id: session_id.to_string(),
            transport_kind: RuntimeTransportKind::Gateway,
            transport_id: gateway_id.to_string(),
            cursor_seq: cursor_seq.map(sequence_to_i64).unwrap_or_default(),
            acked_seq: acked_seq.map(sequence_to_i64).unwrap_or_default(),
            bounded_replay_limit: DEFAULT_BOUNDED_REPLAY_LIMIT,
            last_close_reason: None,
        },
    )
}

/// 读取网关 transport 状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
///
/// 返回:
/// - transport 状态
pub(crate) fn load_gateway_transport_state(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
) -> Result<Option<RuntimeTransportState>> {
    load_transport_state(db, session_id, RuntimeTransportKind::Gateway, gateway_id)
}

/// 记录网关 transport 断开观察事件。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `reason`: 断开原因
/// - `last_sequence`: 最近一次 transport 序号
///
/// 返回:
/// - 写入是否成功
pub(crate) fn record_gateway_transport_close(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    reason: &str,
    last_sequence: Option<u64>,
) -> Result<()> {
    let close_reason = format_transport_reason(gateway_id, reason);
    let state = advance_transport_state(
        db,
        RuntimeTransportStateUpsert {
            session_id: session_id.to_string(),
            transport_kind: RuntimeTransportKind::Gateway,
            transport_id: gateway_id.to_string(),
            cursor_seq: last_sequence.map(sequence_to_i64).unwrap_or_default(),
            acked_seq: 0,
            bounded_replay_limit: DEFAULT_BOUNDED_REPLAY_LIMIT,
            last_close_reason: Some(close_reason.clone()),
        },
    )?;
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: None,
            kind: RuntimeRecoveryKind::TransportClosed,
            status: RuntimeRecoveryStatus::Observed,
            reason: close_reason,
            last_safe_seq: last_sequence.map(|_| state.cursor_seq),
        },
    )?;
    Ok(())
}

/// 推进 Runtime transport 状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `state`: 待写入 transport 状态
///
/// 返回:
/// - 推进后的 transport 状态
fn advance_transport_state(
    db: &ConversationDb,
    state: RuntimeTransportStateUpsert,
) -> Result<RuntimeTransportState> {
    db.with_conn(|conn| {
        let close_observed = state.last_close_reason.is_some();
        let existing = load_transport_state_locked(
            conn,
            &state.session_id,
            &state.transport_kind,
            &state.transport_id,
        )?;
        let cursor_seq = existing
            .as_ref()
            .map(|current| current.cursor_seq)
            .unwrap_or_default()
            .max(state.cursor_seq.max(0));
        let acked_seq = existing
            .as_ref()
            .map(|current| current.acked_seq)
            .unwrap_or_default()
            .max(state.acked_seq.max(0))
            .min(cursor_seq);
        let bounded_replay_limit = state.bounded_replay_limit.max(1);
        let last_close_reason = state.last_close_reason.or_else(|| {
            existing
                .as_ref()
                .and_then(|current| current.last_close_reason.clone())
        });
        let now = Utc::now().to_rfc3339();
        let created_at = existing
            .as_ref()
            .map(|current| current.created_at.clone())
            .unwrap_or_else(|| now.clone());
        let last_closed_at = if close_observed {
            Some(now.clone())
        } else {
            existing.and_then(|current| current.last_closed_at)
        };
        conn.execute(
            "INSERT INTO runtime_transport_state (
                session_id, transport_kind, transport_id, cursor_seq, acked_seq,
                bounded_replay_limit, last_close_reason, last_closed_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(session_id, transport_kind, transport_id) DO UPDATE SET
                cursor_seq = excluded.cursor_seq,
                acked_seq = excluded.acked_seq,
                bounded_replay_limit = excluded.bounded_replay_limit,
                last_close_reason = excluded.last_close_reason,
                last_closed_at = excluded.last_closed_at,
                updated_at = excluded.updated_at",
            params![
                state.session_id,
                state.transport_kind.as_str(),
                state.transport_id,
                cursor_seq,
                acked_seq,
                bounded_replay_limit,
                last_close_reason,
                last_closed_at,
                created_at,
                now,
            ],
        )?;
        load_transport_state_locked(
            conn,
            &state.session_id,
            &state.transport_kind,
            &state.transport_id,
        )?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "runtime transport state was not found after upsert: {}:{}:{}",
                state.session_id,
                state.transport_kind.as_str(),
                state.transport_id
            )
        })
    })
}

/// 读取 Runtime transport 状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `transport_kind`: transport 类型
/// - `transport_id`: transport 标识
///
/// 返回:
/// - transport 状态
fn load_transport_state(
    db: &ConversationDb,
    session_id: &str,
    transport_kind: RuntimeTransportKind,
    transport_id: &str,
) -> Result<Option<RuntimeTransportState>> {
    db.with_conn(|conn| {
        load_transport_state_locked(conn, session_id, &transport_kind, transport_id)
    })
}

/// 读取 Runtime transport 状态。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `transport_kind`: transport 类型
/// - `transport_id`: transport 标识
///
/// 返回:
/// - transport 状态
fn load_transport_state_locked(
    conn: &rusqlite::Connection,
    session_id: &str,
    transport_kind: &RuntimeTransportKind,
    transport_id: &str,
) -> Result<Option<RuntimeTransportState>> {
    conn.query_row(
        "SELECT session_id, transport_kind, transport_id, cursor_seq, acked_seq,
                bounded_replay_limit, last_close_reason, last_closed_at, created_at, updated_at
         FROM runtime_transport_state
         WHERE session_id = ?1 AND transport_kind = ?2 AND transport_id = ?3",
        params![session_id, transport_kind.as_str(), transport_id],
        map_transport_state,
    )
    .optional()
    .map_err(Into::into)
}

/// 从查询行恢复 Runtime transport 状态。
///
/// 参数:
/// - `row`: SQLite 查询行
///
/// 返回:
/// - transport 状态
fn map_transport_state(row: &Row<'_>) -> rusqlite::Result<RuntimeTransportState> {
    let transport_kind: String = row.get(1)?;
    Ok(RuntimeTransportState {
        session_id: row.get(0)?,
        transport_kind: RuntimeTransportKind::from_str(&transport_kind),
        transport_id: row.get(2)?,
        cursor_seq: row.get(3)?,
        acked_seq: row.get(4)?,
        bounded_replay_limit: row.get(5)?,
        last_close_reason: row.get(6)?,
        last_closed_at: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// 格式化 transport 断开原因。
///
/// 参数:
/// - `gateway_id`: 网关标识
/// - `reason`: 原始断开原因
///
/// 返回:
/// - 可写入恢复记录的原因
fn format_transport_reason(gateway_id: &str, reason: &str) -> String {
    let reason = reason.trim();
    let reason = if reason.is_empty() {
        "transport closed"
    } else {
        reason
    };
    let reason = clip_chars(reason, MAX_TRANSPORT_REASON_CHARS);
    format!("gateway_id={gateway_id}; {reason}")
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

/// 按字符数量裁剪字符串。
///
/// 参数:
/// - `value`: 原始字符串
/// - `limit`: 最大字符数量
///
/// 返回:
/// - 裁剪后的字符串
fn clip_chars(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push_str("...");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_recovery::{session_summary, RuntimeRecoveryKind};

    #[test]
    fn records_gateway_transport_close_without_process_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        record_gateway_transport_close(
            &db,
            "default",
            "qq",
            "QQ websocket closed by server",
            Some(42),
        )
        .unwrap();

        let summary = session_summary(&db, "default").unwrap();
        let latest = summary.latest_failure.unwrap();
        assert_eq!(summary.active_process_count, 0);
        assert_eq!(latest.kind, RuntimeRecoveryKind::TransportClosed);
        assert_eq!(latest.last_safe_seq, Some(42));
        assert!(latest.reason.contains("gateway_id=qq"));
        assert!(latest.reason.contains("QQ websocket closed by server"));
        let state = load_gateway_transport_state(&db, "default", "qq")
            .unwrap()
            .unwrap();
        assert_eq!(state.cursor_seq, 42);
        assert_eq!(state.acked_seq, 0);
        assert!(state.last_close_reason.is_some());
    }

    #[test]
    fn gateway_transport_cursor_and_ack_do_not_move_backwards() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        advance_gateway_transport_cursor(&db, "default", "qq", Some(10), Some(8)).unwrap();
        let state =
            advance_gateway_transport_cursor(&db, "default", "qq", Some(6), Some(12)).unwrap();

        assert_eq!(state.cursor_seq, 10);
        assert_eq!(state.acked_seq, 10);
    }

    #[test]
    fn gateway_transport_ack_never_exceeds_cursor() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        let state =
            advance_gateway_transport_cursor(&db, "default", "qq", Some(3), Some(9)).unwrap();

        assert_eq!(state.cursor_seq, 3);
        assert_eq!(state.acked_seq, 3);
    }
}

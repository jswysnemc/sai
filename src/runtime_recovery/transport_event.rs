use super::transport_model::{RuntimeTransportEvent, RuntimeTransportKind};
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Row};
use serde_json::Value;

/// 写入网关 transport 事件到本地 replay source。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `sequence`: transport 事件序号
/// - `payload`: 原始 Gateway Payload
///
/// 返回:
/// - 写入后的 transport 事件
pub(crate) fn record_gateway_transport_event(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    sequence: u64,
    payload: &Value,
) -> Result<RuntimeTransportEvent> {
    record_transport_event(
        db,
        session_id,
        RuntimeTransportKind::Gateway,
        gateway_id,
        sequence_to_i64(sequence),
        payload,
    )
}

/// 读取网关 transport 本地 replay 事件。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `start_sequence`: 起始序号
/// - `end_sequence`: 结束序号
///
/// 返回:
/// - 按序排列的 transport 事件
pub(crate) fn load_gateway_transport_events(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    start_sequence: i64,
    end_sequence: i64,
) -> Result<Vec<RuntimeTransportEvent>> {
    load_transport_events(
        db,
        session_id,
        RuntimeTransportKind::Gateway,
        gateway_id,
        start_sequence,
        end_sequence,
    )
}

/// 判断网关 transport 本地 replay source 是否包含完整区间。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `gateway_id`: 网关标识
/// - `start_sequence`: 起始序号
/// - `end_sequence`: 结束序号
///
/// 返回:
/// - 是否包含完整区间
pub(crate) fn has_gateway_transport_event_range(
    db: &ConversationDb,
    session_id: &str,
    gateway_id: &str,
    start_sequence: i64,
    end_sequence: i64,
) -> Result<bool> {
    has_transport_event_range(
        db,
        session_id,
        RuntimeTransportKind::Gateway,
        gateway_id,
        start_sequence,
        end_sequence,
    )
}

/// 写入 transport 事件到本地 replay source。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `transport_kind`: transport 类型
/// - `transport_id`: transport 标识
/// - `sequence`: transport 事件序号
/// - `payload`: 原始 Payload
///
/// 返回:
/// - 写入后的 transport 事件
fn record_transport_event(
    db: &ConversationDb,
    session_id: &str,
    transport_kind: RuntimeTransportKind,
    transport_id: &str,
    sequence: i64,
    payload: &Value,
) -> Result<RuntimeTransportEvent> {
    let payload_json = serde_json::to_string(payload)?;
    let created_at = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO runtime_transport_events (
                session_id, transport_kind, transport_id, seq, payload_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session_id,
                transport_kind.as_str(),
                transport_id,
                sequence,
                payload_json,
                created_at,
            ],
        )?;
        conn.query_row(
            "SELECT session_id, transport_kind, transport_id, seq, payload_json, created_at
             FROM runtime_transport_events
             WHERE session_id = ?1 AND transport_kind = ?2 AND transport_id = ?3 AND seq = ?4",
            params![session_id, transport_kind.as_str(), transport_id, sequence],
            map_transport_event,
        )
        .map_err(Into::into)
    })
}

/// 读取 transport 事件区间。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `transport_kind`: transport 类型
/// - `transport_id`: transport 标识
/// - `start_sequence`: 起始序号
/// - `end_sequence`: 结束序号
///
/// 返回:
/// - 按序排列的 transport 事件
fn load_transport_events(
    db: &ConversationDb,
    session_id: &str,
    transport_kind: RuntimeTransportKind,
    transport_id: &str,
    start_sequence: i64,
    end_sequence: i64,
) -> Result<Vec<RuntimeTransportEvent>> {
    if start_sequence > end_sequence {
        return Ok(Vec::new());
    }
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT session_id, transport_kind, transport_id, seq, payload_json, created_at
             FROM runtime_transport_events
             WHERE session_id = ?1 AND transport_kind = ?2 AND transport_id = ?3
             AND seq BETWEEN ?4 AND ?5
             ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(
            params![
                session_id,
                transport_kind.as_str(),
                transport_id,
                start_sequence,
                end_sequence,
            ],
            map_transport_event,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    })
}

/// 判断 transport 事件区间是否完整。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `transport_kind`: transport 类型
/// - `transport_id`: transport 标识
/// - `start_sequence`: 起始序号
/// - `end_sequence`: 结束序号
///
/// 返回:
/// - 是否包含完整区间
fn has_transport_event_range(
    db: &ConversationDb,
    session_id: &str,
    transport_kind: RuntimeTransportKind,
    transport_id: &str,
    start_sequence: i64,
    end_sequence: i64,
) -> Result<bool> {
    if start_sequence > end_sequence {
        return Ok(true);
    }
    let expected = end_sequence
        .saturating_sub(start_sequence)
        .saturating_add(1);
    db.with_conn(|conn| {
        let actual: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM runtime_transport_events
             WHERE session_id = ?1 AND transport_kind = ?2 AND transport_id = ?3
             AND seq BETWEEN ?4 AND ?5",
            params![
                session_id,
                transport_kind.as_str(),
                transport_id,
                start_sequence,
                end_sequence,
            ],
            |row| row.get(0),
        )?;
        Ok(actual == expected)
    })
}

/// 从查询行恢复 Runtime transport 事件。
///
/// 参数:
/// - `row`: SQLite 查询行
///
/// 返回:
/// - Runtime transport 事件
fn map_transport_event(row: &Row<'_>) -> rusqlite::Result<RuntimeTransportEvent> {
    let transport_kind: String = row.get(1)?;
    Ok(RuntimeTransportEvent {
        session_id: row.get(0)?,
        transport_kind: RuntimeTransportKind::from_str(&transport_kind),
        transport_id: row.get(2)?,
        sequence: row.get(3)?,
        payload_json: row.get(4)?,
        created_at: row.get(5)?,
    })
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
    use crate::state::ConversationDb;
    use serde_json::json;

    #[test]
    fn gateway_transport_events_record_and_load_range() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();

        record_gateway_transport_event(&db, "default", "qq", 7, &json!({"s": 7})).unwrap();
        record_gateway_transport_event(&db, "default", "qq", 8, &json!({"s": 8})).unwrap();

        assert!(has_gateway_transport_event_range(&db, "default", "qq", 7, 8).unwrap());
        assert!(!has_gateway_transport_event_range(&db, "default", "qq", 7, 9).unwrap());
        let events = load_gateway_transport_events(&db, "default", "qq", 7, 8).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sequence, 7);
        assert_eq!(events[1].sequence, 8);
    }
}

use super::model::{
    FailureKind, NewRecoveryRecord, RecoveryRecord, RecoverySnapshot, RecoveryStatus,
};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::state::turns::ConversationDb;

/// 写入恢复记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入记录
///
/// 返回:
/// - 已写入记录
pub(in crate::state) fn insert_record(
    db: &ConversationDb,
    record: NewRecoveryRecord,
) -> Result<RecoveryRecord> {
    let conn = db.conn.lock().unwrap();
    if let Some(existing) = find_unresolved_duplicate_locked(&conn, &record)? {
        return Ok(existing);
    }
    let created_at = Utc::now().to_rfc3339();
    let record = RecoveryRecord {
        id: new_record_id(),
        session_id: record.session_id,
        turn_id: record.turn_id,
        kind: record.kind,
        status: record.status,
        reason: record.reason,
        retry_count: record.retry_count,
        checkpoint_id: record.checkpoint_id,
        context_chars: record.context_chars,
        context_limit_chars: record.context_limit_chars,
        created_at,
        resolved_at: None,
    };
    insert_record_locked(&conn, &record)?;
    Ok(record)
}

/// 读取会话恢复快照。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `auto_compaction_threshold`: 自动压缩熔断阈值
///
/// 返回:
/// - 恢复快照
pub(in crate::state) fn snapshot(
    db: &ConversationDb,
    session_id: &str,
    auto_compaction_threshold: usize,
) -> Result<RecoverySnapshot> {
    let conn = db.conn.lock().unwrap();
    let latest = latest_record_locked(&conn, session_id)?;
    let auto_compaction_failures = active_auto_compaction_failures_locked(&conn, session_id)?;
    let stale_turns_recovered = stale_turns_recovered_locked(&conn, session_id)?;
    Ok(RecoverySnapshot {
        latest,
        auto_compaction_failures,
        auto_compaction_blocked: auto_compaction_failures >= auto_compaction_threshold,
        stale_turns_recovered,
    })
}

/// 读取最近 checkpoint id。
///
/// 参数:
/// - `db`: 对话数据库
///
/// 返回:
/// - 最近 checkpoint id
pub(in crate::state) fn latest_checkpoint_id(db: &ConversationDb) -> Result<Option<String>> {
    let conn = db.conn.lock().unwrap();
    latest_checkpoint_id_locked(&conn)
}

/// 标记活跃压缩失败已恢复。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 更新数量
pub(in crate::state) fn resolve_active_compaction_failures(
    db: &ConversationDb,
    session_id: &str,
) -> Result<usize> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE failure_recovery_records
         SET status = 'resolved', resolved_at = ?1
         WHERE session_id = ?2
           AND kind IN (
               'compaction_llm_failed',
               'empty_summary',
               'compaction_over_budget',
               'session_memory_compact_failed'
           )
           AND status != 'resolved'",
        params![now, session_id],
    )?;
    Ok(affected)
}

/// 查找未解决的重复恢复记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `record`: 待写入恢复记录
///
/// 返回:
/// - 已存在的恢复记录
fn find_unresolved_duplicate_locked(
    conn: &Connection,
    record: &NewRecoveryRecord,
) -> Result<Option<RecoveryRecord>> {
    conn.query_row(
        "SELECT id, session_id, turn_id, kind, status, reason, retry_count,
                checkpoint_id, context_chars, context_limit_chars, created_at, resolved_at
         FROM failure_recovery_records
         WHERE session_id = ?1
           AND (turn_id IS ?2 OR turn_id = ?2)
           AND kind = ?3
           AND status = ?4
           AND reason = ?5
           AND retry_count = ?6
           AND (checkpoint_id IS ?7 OR checkpoint_id = ?7)
           AND context_chars = ?8
           AND context_limit_chars = ?9
           AND status != 'resolved'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
        params![
            record.session_id,
            record.turn_id,
            record.kind.as_str(),
            record.status.as_str(),
            record.reason,
            record.retry_count as i64,
            record.checkpoint_id,
            record.context_chars as i64,
            record.context_limit_chars as i64,
        ],
        map_record,
    )
    .optional()
    .map_err(Into::into)
}

/// 插入恢复记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `record`: 恢复记录
///
/// 返回:
/// - 写入是否成功
fn insert_record_locked(conn: &Connection, record: &RecoveryRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO failure_recovery_records (
            id, session_id, turn_id, kind, status, reason, retry_count,
            checkpoint_id, context_chars, context_limit_chars, created_at, resolved_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            record.id,
            record.session_id,
            record.turn_id,
            record.kind.as_str(),
            record.status.as_str(),
            record.reason,
            record.retry_count as i64,
            record.checkpoint_id,
            record.context_chars as i64,
            record.context_limit_chars as i64,
            record.created_at,
            record.resolved_at,
        ],
    )?;
    Ok(())
}

/// 读取最近恢复记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 最近恢复记录
fn latest_record_locked(conn: &Connection, session_id: &str) -> Result<Option<RecoveryRecord>> {
    conn.query_row(
        "SELECT id, session_id, turn_id, kind, status, reason, retry_count,
                checkpoint_id, context_chars, context_limit_chars, created_at, resolved_at
         FROM failure_recovery_records
         WHERE session_id = ?1
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
        params![session_id],
        map_record,
    )
    .optional()
    .map_err(Into::into)
}

/// 读取活跃自动压缩失败次数。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 连续失败次数
fn active_auto_compaction_failures_locked(conn: &Connection, session_id: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COALESCE(MAX(retry_count), 0)
         FROM failure_recovery_records
         WHERE session_id = ?1
           AND kind IN (
               'compaction_llm_failed',
               'empty_summary',
               'compaction_over_budget',
               'session_memory_compact_failed'
           )
           AND status != 'resolved'",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// 统计 stale turn 恢复数量。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 已恢复轮次数量
fn stale_turns_recovered_locked(conn: &Connection, session_id: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM failure_recovery_records
         WHERE session_id = ?1
           AND kind = 'stale_running_turn'",
        params![session_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// 读取最近 checkpoint id。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 最近 checkpoint id
fn latest_checkpoint_id_locked(conn: &Connection) -> Result<Option<String>> {
    conn.query_row(
        "SELECT id FROM compaction_checkpoints ORDER BY seq DESC LIMIT 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

/// 从查询行恢复记录。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 恢复记录
fn map_record(row: &Row<'_>) -> rusqlite::Result<RecoveryRecord> {
    let kind: String = row.get(3)?;
    let status: String = row.get(4)?;
    Ok(RecoveryRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        turn_id: row.get(2)?,
        kind: FailureKind::from_str(&kind),
        status: RecoveryStatus::from_str(&status),
        reason: row.get(5)?,
        retry_count: row.get::<_, i64>(6)? as usize,
        checkpoint_id: row.get(7)?,
        context_chars: row.get::<_, i64>(8)? as usize,
        context_limit_chars: row.get::<_, i64>(9)? as usize,
        created_at: row.get(10)?,
        resolved_at: row.get(11)?,
    })
}

/// 创建恢复记录标识。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 记录标识
fn new_record_id() -> String {
    format!(
        "fr_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::checkpoints::schema::create_checkpoint_tables;
    use crate::state::failure_recovery::schema::create_failure_recovery_tables;

    #[test]
    fn inserts_and_reads_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        let checkpoint_conn = rusqlite::Connection::open_in_memory().unwrap();
        create_checkpoint_tables(&checkpoint_conn).unwrap();
        create_failure_recovery_tables(&checkpoint_conn).unwrap();

        let first = NewRecoveryRecord {
            session_id: "default".to_string(),
            turn_id: Some("turn_1".to_string()),
            kind: FailureKind::CompactionLlmFailed,
            status: RecoveryStatus::Observed,
            reason: "provider error".to_string(),
            retry_count: 1,
            checkpoint_id: None,
            context_chars: 900,
            context_limit_chars: 1_000,
        };
        insert_record(&db, first).unwrap();

        let snapshot = snapshot(&db, "default", 3).unwrap();

        assert_eq!(snapshot.auto_compaction_failures, 1);
        assert!(!snapshot.auto_compaction_blocked);
        assert_eq!(snapshot.latest.unwrap().reason, "provider error");
    }

    #[test]
    fn insert_record_reuses_unresolved_duplicate() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        let record = NewRecoveryRecord {
            session_id: "default".to_string(),
            turn_id: Some("turn_1".to_string()),
            kind: FailureKind::ProjectionInvalid,
            status: RecoveryStatus::Terminal,
            reason: "duplicate tool result".to_string(),
            retry_count: 0,
            checkpoint_id: None,
            context_chars: 900,
            context_limit_chars: 1_000,
        };

        let first = insert_record(&db, record.clone()).unwrap();
        let second = insert_record(&db, record).unwrap();
        let count: i64 = db
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM failure_recovery_records", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(count, 1);
    }
}

use super::model::{
    NewRuntimeProcessEventInput, NewRuntimeProcessEventRecord, NewRuntimeProcessRecord,
    NewRuntimeRecoveryRecord, RuntimeProcessEventRecord, RuntimeProcessRecord,
    RuntimeProcessStatus, RuntimeRecoveryKind, RuntimeRecoveryRecord, RuntimeRecoveryStatus,
};
use super::summary::{RuntimeRecoveryFailureSummary, RuntimeRecoverySummary};
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 写入或更新运行时进程记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `process`: 待写入运行时进程
///
/// 返回:
/// - 写入后的运行时进程记录
pub(crate) fn record_process(
    db: &ConversationDb,
    process: NewRuntimeProcessRecord,
) -> Result<RuntimeProcessRecord> {
    db.with_conn(|conn| {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO runtime_processes (
                id, session_id, owner_kind, owner_id, process_kind, command, cwd,
                pid, pgid, status, last_seq, last_seen_at, started_at, updated_at, ended_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13, NULL)
             ON CONFLICT(id) DO UPDATE SET
                owner_kind = excluded.owner_kind,
                owner_id = excluded.owner_id,
                process_kind = excluded.process_kind,
                command = excluded.command,
                cwd = excluded.cwd,
                pid = excluded.pid,
                pgid = excluded.pgid,
                status = excluded.status,
                last_seq = MAX(runtime_processes.last_seq, excluded.last_seq),
                last_seen_at = excluded.last_seen_at,
                updated_at = excluded.updated_at,
                ended_at = CASE
                    WHEN excluded.status IN ('exited', 'stopped', 'failed') THEN excluded.updated_at
                    ELSE runtime_processes.ended_at
                END",
            params![
                process.id,
                process.session_id,
                process.owner_kind.as_str(),
                process.owner_id,
                process.process_kind.as_str(),
                process.command,
                process.cwd,
                process.pid,
                process.pgid,
                process.status.as_str(),
                process.last_seq,
                now,
                now,
            ],
        )?;
        load_process_locked(conn, &process.id)?.ok_or_else(|| {
            anyhow::anyhow!("runtime process was not found after upsert: {}", process.id)
        })
    })
}

/// 追加运行时进程事件。
///
/// 参数:
/// - `db`: 对话数据库
/// - `event`: 待写入事件
///
/// 返回:
/// - 写入后的进程事件
#[allow(dead_code)]
pub(crate) fn append_process_event(
    db: &ConversationDb,
    event: NewRuntimeProcessEventRecord,
) -> Result<RuntimeProcessEventRecord> {
    db.with_conn(|conn| insert_process_event_locked(conn, event))
}

/// 按进程当前序号追加下一条运行时进程事件。
///
/// 参数:
/// - `db`: 对话数据库
/// - `event`: 待写入事件
///
/// 返回:
/// - 写入后的进程事件
pub(crate) fn append_next_process_event(
    db: &ConversationDb,
    event: NewRuntimeProcessEventInput,
) -> Result<RuntimeProcessEventRecord> {
    db.with_conn(|conn| {
        let seq = next_process_seq_locked(conn, &event.process_id)?;
        insert_process_event_locked(
            conn,
            NewRuntimeProcessEventRecord {
                process_id: event.process_id,
                seq,
                stream: event.stream,
                event_kind: event.event_kind,
                payload_ref: event.payload_ref,
                payload_preview: event.payload_preview,
            },
        )
    })
}

/// 写入运行时进程事件并推进进程序号。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `event`: 待写入事件
///
/// 返回:
/// - 写入后的进程事件
fn insert_process_event_locked(
    conn: &Connection,
    event: NewRuntimeProcessEventRecord,
) -> Result<RuntimeProcessEventRecord> {
    let created_at = Utc::now().to_rfc3339();
    let id = new_record_id("rpe");
    conn.execute(
        "INSERT INTO runtime_process_events (
            id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            &event.process_id,
            event.seq,
            event.stream,
            event.event_kind,
            event.payload_ref,
            event.payload_preview,
            created_at,
        ],
    )?;
    conn.execute(
        "UPDATE runtime_processes
         SET last_seq = MAX(last_seq, ?1), last_seen_at = ?2, updated_at = ?2
         WHERE id = ?3",
        params![event.seq, created_at, &event.process_id],
    )?;
    load_event_locked(conn, &id)?
        .ok_or_else(|| anyhow::anyhow!("runtime process event was not found after insert: {id}"))
}

/// 读取进程下一条事件序号。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `process_id`: 进程记录标识
///
/// 返回:
/// - 下一条事件序号
fn next_process_seq_locked(conn: &Connection, process_id: &str) -> Result<i64> {
    let last_seq: Option<i64> = conn
        .query_row(
            "SELECT last_seq FROM runtime_processes WHERE id = ?1",
            params![process_id],
            |row| row.get(0),
        )
        .optional()?;
    last_seq
        .map(|seq| seq + 1)
        .ok_or_else(|| anyhow::anyhow!("runtime process was not found: {process_id}"))
}

/// 写入运行时恢复记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入恢复记录
///
/// 返回:
/// - 写入后的恢复记录
pub(crate) fn record_recovery(
    db: &ConversationDb,
    record: NewRuntimeRecoveryRecord,
) -> Result<RuntimeRecoveryRecord> {
    db.with_conn(|conn| {
        if let Some(existing) = find_unresolved_recovery_duplicate_locked(conn, &record)? {
            return Ok(existing);
        }
        let created_at = Utc::now().to_rfc3339();
        let id = new_record_id("rr");
        conn.execute(
            "INSERT INTO runtime_recovery_records (
                id, session_id, process_id, kind, status, reason, last_safe_seq, created_at, resolved_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
            params![
                id,
                record.session_id,
                record.process_id,
                record.kind.as_str(),
                record.status.as_str(),
                record.reason,
                record.last_safe_seq,
                created_at,
            ],
        )?;
        load_recovery_locked(conn, &id)?.ok_or_else(|| {
            anyhow::anyhow!("runtime recovery record was not found after insert: {id}")
        })
    })
}

/// 查找完全相同的未解决运行时恢复记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `record`: 待写入恢复记录
///
/// 返回:
/// - 已存在的未解决恢复记录
fn find_unresolved_recovery_duplicate_locked(
    conn: &Connection,
    record: &NewRuntimeRecoveryRecord,
) -> Result<Option<RuntimeRecoveryRecord>> {
    conn.query_row(
        "SELECT id, session_id, process_id, kind, status, reason, last_safe_seq, created_at, resolved_at
         FROM runtime_recovery_records
         WHERE session_id = ?1
         AND process_id IS ?2
         AND kind = ?3
         AND status = ?4
         AND reason = ?5
         AND last_safe_seq IS ?6
         AND status != ?7
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
        params![
            &record.session_id,
            &record.process_id,
            record.kind.as_str(),
            record.status.as_str(),
            &record.reason,
            &record.last_safe_seq,
            RuntimeRecoveryStatus::Resolved.as_str(),
        ],
        map_recovery,
    )
    .optional()
    .map_err(Into::into)
}

/// 读取会话 Runtime Recovery 摘要。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - Runtime Recovery 摘要
pub(crate) fn session_summary(
    db: &ConversationDb,
    session_id: &str,
) -> Result<RuntimeRecoverySummary> {
    db.with_conn(|conn| {
        let active_process_count =
            count_processes_by_status_locked(conn, session_id, RuntimeProcessStatus::Running)?;
        let stale_process_count =
            count_processes_by_status_locked(conn, session_id, RuntimeProcessStatus::Stale)?;
        let latest_failure = latest_failure_locked(conn, session_id)?;
        Ok(RuntimeRecoverySummary {
            active_process_count,
            stale_process_count,
            latest_failure,
        })
    })
}

/// 按状态统计进程数量。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `status`: 进程状态
///
/// 返回:
/// - 进程数量
fn count_processes_by_status_locked(
    conn: &Connection,
    session_id: &str,
    status: RuntimeProcessStatus,
) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*)
         FROM runtime_processes
         WHERE session_id = ?1 AND status = ?2",
        params![session_id, status.as_str()],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// 读取最近未解决运行时恢复失败。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 最近失败摘要
fn latest_failure_locked(
    conn: &Connection,
    session_id: &str,
) -> Result<Option<RuntimeRecoveryFailureSummary>> {
    conn.query_row(
        "SELECT process_id, kind, status, reason, last_safe_seq, created_at
         FROM runtime_recovery_records
         WHERE session_id = ?1 AND status != 'resolved'
         ORDER BY created_at DESC, id DESC
         LIMIT 1",
        params![session_id],
        |row| {
            let kind: String = row.get(1)?;
            let status: String = row.get(2)?;
            Ok(RuntimeRecoveryFailureSummary {
                process_id: row.get(0)?,
                kind: RuntimeRecoveryKind::from_str(&kind),
                status: RuntimeRecoveryStatus::from_str(&status),
                reason: row.get(3)?,
                last_safe_seq: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// 读取运行时进程。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `id`: 进程记录标识
///
/// 返回:
/// - 运行时进程记录
fn load_process_locked(conn: &Connection, id: &str) -> Result<Option<RuntimeProcessRecord>> {
    conn.query_row(
        "SELECT id, session_id, owner_kind, owner_id, process_kind, command, cwd,
                pid, pgid, status, last_seq, last_seen_at, started_at, updated_at, ended_at
         FROM runtime_processes
         WHERE id = ?1",
        params![id],
        map_process,
    )
    .optional()
    .map_err(Into::into)
}

/// 读取运行时进程事件。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `id`: 事件标识
///
/// 返回:
/// - 运行时进程事件
fn load_event_locked(conn: &Connection, id: &str) -> Result<Option<RuntimeProcessEventRecord>> {
    conn.query_row(
        "SELECT id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
         FROM runtime_process_events
         WHERE id = ?1",
        params![id],
        map_event,
    )
    .optional()
    .map_err(Into::into)
}

/// 读取运行时恢复记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `id`: 恢复记录标识
///
/// 返回:
/// - 运行时恢复记录
fn load_recovery_locked(conn: &Connection, id: &str) -> Result<Option<RuntimeRecoveryRecord>> {
    conn.query_row(
        "SELECT id, session_id, process_id, kind, status, reason, last_safe_seq, created_at, resolved_at
         FROM runtime_recovery_records
         WHERE id = ?1",
        params![id],
        map_recovery,
    )
    .optional()
    .map_err(Into::into)
}

/// 从查询行恢复进程记录。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 运行时进程记录
fn map_process(row: &Row<'_>) -> rusqlite::Result<RuntimeProcessRecord> {
    let owner_kind: String = row.get(2)?;
    let process_kind: String = row.get(4)?;
    let status: String = row.get(9)?;
    Ok(RuntimeProcessRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        owner_kind: super::model::OwnerKind::from_str(&owner_kind),
        owner_id: row.get(3)?,
        process_kind: super::model::ProcessKind::from_str(&process_kind),
        command: row.get(5)?,
        cwd: row.get(6)?,
        pid: row.get(7)?,
        pgid: row.get(8)?,
        status: RuntimeProcessStatus::from_str(&status),
        last_seq: row.get(10)?,
        last_seen_at: row.get(11)?,
        started_at: row.get(12)?,
        updated_at: row.get(13)?,
        ended_at: row.get(14)?,
    })
}

/// 从查询行恢复进程事件。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 运行时进程事件
fn map_event(row: &Row<'_>) -> rusqlite::Result<RuntimeProcessEventRecord> {
    Ok(RuntimeProcessEventRecord {
        id: row.get(0)?,
        process_id: row.get(1)?,
        seq: row.get(2)?,
        stream: row.get(3)?,
        event_kind: row.get(4)?,
        payload_ref: row.get(5)?,
        payload_preview: row.get(6)?,
        created_at: row.get(7)?,
    })
}

/// 从查询行恢复运行时恢复记录。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 运行时恢复记录
fn map_recovery(row: &Row<'_>) -> rusqlite::Result<RuntimeRecoveryRecord> {
    let kind: String = row.get(3)?;
    let status: String = row.get(4)?;
    Ok(RuntimeRecoveryRecord {
        id: row.get(0)?,
        session_id: row.get(1)?,
        process_id: row.get(2)?,
        kind: RuntimeRecoveryKind::from_str(&kind),
        status: RuntimeRecoveryStatus::from_str(&status),
        reason: row.get(5)?,
        last_safe_seq: row.get(6)?,
        created_at: row.get(7)?,
        resolved_at: row.get(8)?,
    })
}

/// 创建运行时记录标识。
///
/// 参数:
/// - `prefix`: 标识前缀
///
/// 返回:
/// - 运行时记录标识
fn new_record_id(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_recovery::model::{
        OwnerKind, ProcessKind, RuntimeProcessStatus, RuntimeRecoveryKind, RuntimeRecoveryStatus,
    };

    fn db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    #[test]
    fn records_process_event_and_summary() {
        let (_temp, db) = db();
        let process = record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_1".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::CommandMode,
                owner_id: "command".to_string(),
                process_kind: ProcessKind::BackgroundCommand,
                command: "sleep 60".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(123),
                pgid: Some(123),
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();

        let event = append_process_event(
            &db,
            NewRuntimeProcessEventRecord {
                process_id: process.id.clone(),
                seq: 1,
                stream: "stdout".to_string(),
                event_kind: "output".to_string(),
                payload_ref: None,
                payload_preview: "hello".to_string(),
            },
        )
        .unwrap();
        let summary = session_summary(&db, "default").unwrap();

        assert_eq!(event.seq, 1);
        assert_eq!(summary.active_process_count, 1);
        assert_eq!(summary.stale_process_count, 0);
        assert!(summary.latest_failure.is_none());
    }

    #[test]
    fn summary_reports_stale_process_and_latest_failure() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_1".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Session,
                owner_id: "default".to_string(),
                process_kind: ProcessKind::BackgroundCommand,
                command: "sleep 60".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(123),
                pgid: Some(123),
                status: RuntimeProcessStatus::Stale,
                last_seq: 7,
            },
        )
        .unwrap();
        record_recovery(
            &db,
            NewRuntimeRecoveryRecord {
                session_id: "default".to_string(),
                process_id: Some("proc_1".to_string()),
                kind: RuntimeRecoveryKind::SequenceGap,
                status: RuntimeRecoveryStatus::Terminal,
                reason: "missing seq 8".to_string(),
                last_safe_seq: Some(7),
            },
        )
        .unwrap();

        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();

        assert_eq!(summary.active_process_count, 0);
        assert_eq!(summary.stale_process_count, 1);
        assert_eq!(failure.process_id.as_deref(), Some("proc_1"));
        assert_eq!(failure.last_safe_seq, Some(7));
        assert!(failure.reason.contains("missing seq 8"));
    }

    #[test]
    fn record_recovery_reuses_identical_unresolved_record() {
        let (_temp, db) = db();
        let input = NewRuntimeRecoveryRecord {
            session_id: "default".to_string(),
            process_id: Some("proc_1".to_string()),
            kind: RuntimeRecoveryKind::OutputCapReached,
            status: RuntimeRecoveryStatus::Observed,
            reason: "background command stdout output exceeded 20000 bytes".to_string(),
            last_safe_seq: Some(3),
        };

        let first = record_recovery(&db, input.clone()).unwrap();
        let second = record_recovery(&db, input).unwrap();
        let count: i64 = db
            .with_conn(|conn| {
                Ok(
                    conn.query_row("SELECT COUNT(*) FROM runtime_recovery_records", [], |row| {
                        row.get(0)
                    })?,
                )
            })
            .unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(count, 1);
        assert_eq!(failure.reason, first.reason);
        assert_eq!(failure.created_at, first.created_at);
    }

    #[test]
    fn process_sync_does_not_move_last_seq_backwards() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_1".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Session,
                owner_id: "default".to_string(),
                process_kind: ProcessKind::BackgroundCommand,
                command: "sleep 60".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(123),
                pgid: Some(123),
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();
        append_process_event(
            &db,
            NewRuntimeProcessEventRecord {
                process_id: "proc_1".to_string(),
                seq: 3,
                stream: "stdout".to_string(),
                event_kind: "output_read".to_string(),
                payload_ref: None,
                payload_preview: "tail".to_string(),
            },
        )
        .unwrap();

        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_1".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Session,
                owner_id: "default".to_string(),
                process_kind: ProcessKind::BackgroundCommand,
                command: "sleep 60".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(123),
                pgid: Some(123),
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();

        let process = db
            .with_conn(|conn| load_process_locked(conn, "proc_1"))
            .unwrap()
            .unwrap();
        assert_eq!(process.last_seq, 3);
    }
}

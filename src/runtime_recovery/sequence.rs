use super::model::{RuntimeRecoveryKind, RuntimeRecoveryStatus};
use super::replay::{insert_replay_unavailable_marker_locked, try_insert_log_tail_replay_locked};
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

struct ProcessSequenceCursor {
    id: String,
    last_seq: i64,
}

/// 审计会话内运行时进程事件序列缺口。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 新写入的恢复记录数量
pub(crate) fn audit_sequence_gaps(db: &ConversationDb, session_id: &str) -> Result<usize> {
    db.with_conn(|conn| {
        let processes = load_process_sequence_cursors_locked(conn, session_id)?;
        let mut inserted = 0;
        for process in processes {
            let Some(missing_seq) =
                first_missing_sequence_locked(conn, &process.id, process.last_seq)?
            else {
                continue;
            };
            if unresolved_sequence_gap_exists_locked(conn, session_id, &process.id)? {
                continue;
            }
            if !try_insert_log_tail_replay_locked(conn, &process.id, missing_seq)? {
                insert_replay_unavailable_marker_locked(conn, &process.id, missing_seq)?;
            }
            insert_sequence_gap_locked(conn, session_id, &process.id, missing_seq)?;
            inserted += 1;
        }
        Ok(inserted)
    })
}

/// 读取会话内需要审计的进程序列游标。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 进程序列游标列表
fn load_process_sequence_cursors_locked(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<ProcessSequenceCursor>> {
    let mut stmt = conn.prepare(
        "SELECT id, last_seq
         FROM runtime_processes
         WHERE session_id = ?1
         AND last_seq > 0
         ORDER BY updated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(ProcessSequenceCursor {
            id: row.get(0)?,
            last_seq: row.get(1)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// 读取进程事件序列的第一个缺口。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `process_id`: 进程记录标识
/// - `last_seq`: 进程最后安全序号
///
/// 返回:
/// - 第一个缺失序号
fn first_missing_sequence_locked(
    conn: &Connection,
    process_id: &str,
    last_seq: i64,
) -> Result<Option<i64>> {
    let mut expected = 1;
    let mut stmt = conn.prepare(
        "SELECT seq
         FROM runtime_process_events
         WHERE process_id = ?1
         AND seq BETWEEN 1 AND ?2
         ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map(params![process_id, last_seq], |row| row.get::<_, i64>(0))?;
    for seq in rows {
        let seq = seq?;
        if seq == expected {
            expected += 1;
        } else if seq > expected {
            return Ok(Some(expected));
        }
    }
    if expected <= last_seq {
        Ok(Some(expected))
    } else {
        Ok(None)
    }
}

/// 判断进程是否已有未解决序列缺口。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `process_id`: 进程记录标识
///
/// 返回:
/// - 是否已有未解决缺口
fn unresolved_sequence_gap_exists_locked(
    conn: &Connection,
    session_id: &str,
    process_id: &str,
) -> Result<bool> {
    let id: Option<String> = conn
        .query_row(
            "SELECT id
             FROM runtime_recovery_records
             WHERE session_id = ?1
             AND process_id = ?2
             AND kind = ?3
             AND status != ?4
             LIMIT 1",
            params![
                session_id,
                process_id,
                RuntimeRecoveryKind::SequenceGap.as_str(),
                RuntimeRecoveryStatus::Resolved.as_str()
            ],
            |row| row.get(0),
        )
        .optional()?;
    Ok(id.is_some())
}

/// 写入序列缺口恢复记录。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `process_id`: 进程记录标识
/// - `missing_seq`: 缺失序号
///
/// 返回:
/// - 写入是否成功
fn insert_sequence_gap_locked(
    conn: &Connection,
    session_id: &str,
    process_id: &str,
    missing_seq: i64,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO runtime_recovery_records (
            id, session_id, process_id, kind, status, reason, last_safe_seq, created_at, resolved_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL)",
        params![
            new_sequence_record_id(),
            session_id,
            process_id,
            RuntimeRecoveryKind::SequenceGap.as_str(),
            RuntimeRecoveryStatus::Terminal.as_str(),
            format!(
                "runtime process event sequence gap: missing seq {missing_seq}; read-after replay unavailable"
            ),
            missing_seq.saturating_sub(1),
            now,
        ],
    )?;
    Ok(())
}

/// 创建序列审计恢复记录标识。
///
/// 返回:
/// - 恢复记录标识
fn new_sequence_record_id() -> String {
    format!(
        "rr_seq_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_recovery::model::{
        NewRuntimeProcessEventRecord, NewRuntimeProcessRecord, OwnerKind, ProcessKind,
        RuntimeProcessStatus, RuntimeRecoveryKind,
    };
    use crate::runtime_recovery::repository::{
        append_process_event, record_process, session_summary,
    };

    fn db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    #[test]
    fn audit_records_sequence_gap_once() {
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
                seq: 1,
                stream: "stdout".to_string(),
                event_kind: "output_read".to_string(),
                payload_ref: None,
                payload_preview: "one".to_string(),
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
                payload_preview: "three".to_string(),
            },
        )
        .unwrap();

        assert_eq!(audit_sequence_gaps(&db, "default").unwrap(), 1);
        assert_eq!(audit_sequence_gaps(&db, "default").unwrap(), 0);
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();

        assert_eq!(failure.process_id.as_deref(), Some("proc_1"));
        assert_eq!(failure.kind, RuntimeRecoveryKind::SequenceGap);
        assert_eq!(failure.last_safe_seq, Some(1));
        assert!(failure.reason.contains("missing seq 2"));
    }

    #[test]
    fn audit_inserts_replay_unavailable_marker_for_missing_sequence() {
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
                seq: 1,
                stream: "stdout".to_string(),
                event_kind: "output_read".to_string(),
                payload_ref: None,
                payload_preview: "one".to_string(),
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
                payload_preview: "three".to_string(),
            },
        )
        .unwrap();

        assert_eq!(audit_sequence_gaps(&db, "default").unwrap(), 1);
        let marker: (String, String, String) = db
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT stream, event_kind, payload_preview
                     FROM runtime_process_events
                     WHERE process_id = 'proc_1'
                     AND seq = 2",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?)
            })
            .unwrap();

        assert_eq!(marker.0, "recovery");
        assert_eq!(marker.1, "replay_unavailable");
        assert!(marker.2.contains("missing seq 2"));
    }

    #[test]
    fn audit_replays_missing_sequence_from_retained_log_ref() {
        let (temp, db) = db();
        let stdout_log = temp.path().join("stdout.log");
        std::fs::write(&stdout_log, "alpha\nbeta\ngamma\n").unwrap();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_1".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Session,
                owner_id: "default".to_string(),
                process_kind: ProcessKind::BackgroundCommand,
                command: "printf lines".to_string(),
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
                seq: 1,
                stream: "stdout".to_string(),
                event_kind: "output_read".to_string(),
                payload_ref: Some(stdout_log.display().to_string()),
                payload_preview: "alpha".to_string(),
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
                payload_ref: Some(stdout_log.display().to_string()),
                payload_preview: "gamma".to_string(),
            },
        )
        .unwrap();

        assert_eq!(audit_sequence_gaps(&db, "default").unwrap(), 1);
        let replay: (String, String, String, Option<String>) = db
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT stream, event_kind, payload_preview, payload_ref
                     FROM runtime_process_events
                     WHERE process_id = 'proc_1'
                     AND seq = 2",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?)
            })
            .unwrap();

        assert_eq!(replay.0, "stdout");
        assert_eq!(replay.1, "log_tail_replay");
        assert!(replay.2.contains("beta"));
        assert_eq!(replay.3.as_deref(), Some(stdout_log.to_str().unwrap()));
    }
}

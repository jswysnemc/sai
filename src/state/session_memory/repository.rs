use super::model::{NewSessionMemory, SessionMemory};
use anyhow::Result;
use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::state::turns::ConversationDb;

/// 写入或更新会话工作记忆。
///
/// 参数:
/// - `db`: 对话数据库
/// - `memory`: 待写入会话工作记忆
///
/// 返回:
/// - 写入后的会话工作记忆
pub(in crate::state) fn upsert_memory(
    db: &ConversationDb,
    memory: NewSessionMemory,
) -> Result<SessionMemory> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO session_memory (
            session_id, summary, last_summarized_turn_id, last_summarized_seq,
            checkpoint_id, source_turn_count, token_estimate, consecutive_failures,
            disabled_until, last_error, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, NULL, ?8, ?8)
         ON CONFLICT(session_id) DO UPDATE SET
            summary = excluded.summary,
            last_summarized_turn_id = excluded.last_summarized_turn_id,
            last_summarized_seq = excluded.last_summarized_seq,
            checkpoint_id = excluded.checkpoint_id,
            source_turn_count = excluded.source_turn_count,
            token_estimate = excluded.token_estimate,
            consecutive_failures = 0,
            disabled_until = NULL,
            last_error = NULL,
            updated_at = excluded.updated_at",
        params![
            memory.session_id,
            memory.summary,
            memory.last_summarized_turn_id,
            memory.last_summarized_seq,
            memory.checkpoint_id,
            memory.source_turn_count as i64,
            memory.token_estimate as i64,
            now,
        ],
    )?;
    load_memory_locked(&conn, &memory.session_id)?.ok_or_else(|| {
        anyhow::anyhow!(
            "session memory was not found after upsert: {}",
            memory.session_id
        )
    })
}

/// 读取会话工作记忆。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 会话工作记忆
pub(in crate::state) fn load_memory(
    db: &ConversationDb,
    session_id: &str,
) -> Result<Option<SessionMemory>> {
    let conn = db.conn.lock().unwrap();
    load_memory_locked(&conn, session_id)
}

/// 记录会话工作记忆提取失败。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `error`: 失败原因
/// - `failure_threshold`: 触发熔断的连续失败次数
/// - `disable_seconds`: 熔断持续秒数
///
/// 返回:
/// - 更新后的会话工作记忆
pub(in crate::state) fn record_failure(
    db: &ConversationDb,
    session_id: &str,
    error: &str,
    failure_threshold: usize,
    disable_seconds: i64,
) -> Result<SessionMemory> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now();
    let previous = load_memory_locked(&conn, session_id)?;
    let consecutive_failures = previous
        .as_ref()
        .map(|memory| memory.consecutive_failures + 1)
        .unwrap_or(1);
    let disabled_until = (consecutive_failures >= failure_threshold)
        .then(|| (now + Duration::seconds(disable_seconds)).to_rfc3339());
    match previous {
        Some(_) => {
            conn.execute(
                "UPDATE session_memory
                 SET consecutive_failures = ?1,
                     disabled_until = ?2,
                     last_error = ?3,
                     updated_at = ?4
                 WHERE session_id = ?5",
                params![
                    consecutive_failures as i64,
                    disabled_until,
                    error,
                    now.to_rfc3339(),
                    session_id,
                ],
            )?;
        }
        None => {
            conn.execute(
                "INSERT INTO session_memory (
                    session_id, summary, last_summarized_turn_id, last_summarized_seq,
                    checkpoint_id, source_turn_count, token_estimate, consecutive_failures,
                    disabled_until, last_error, created_at, updated_at
                 ) VALUES (?1, '', NULL, 0, NULL, 0, 0, ?2, ?3, ?4, ?5, ?5)",
                params![
                    session_id,
                    consecutive_failures as i64,
                    disabled_until,
                    error,
                    now.to_rfc3339(),
                ],
            )?;
        }
    }
    load_memory_locked(&conn, session_id)?.ok_or_else(|| {
        anyhow::anyhow!(
            "session memory was not found after recording failure: {}",
            session_id
        )
    })
}

/// 读取会话工作记忆。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 会话工作记忆
fn load_memory_locked(conn: &Connection, session_id: &str) -> Result<Option<SessionMemory>> {
    conn.query_row(
        "SELECT session_id, summary, last_summarized_turn_id, last_summarized_seq,
                checkpoint_id, source_turn_count, token_estimate, consecutive_failures,
                disabled_until, last_error, created_at, updated_at
         FROM session_memory
         WHERE session_id = ?1",
        params![session_id],
        map_memory,
    )
    .optional()
    .map_err(Into::into)
}

/// 从查询行恢复会话工作记忆。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 会话工作记忆
fn map_memory(row: &Row<'_>) -> rusqlite::Result<SessionMemory> {
    Ok(SessionMemory {
        session_id: row.get(0)?,
        summary: row.get(1)?,
        last_summarized_turn_id: row.get(2)?,
        last_summarized_seq: row.get(3)?,
        checkpoint_id: row.get(4)?,
        source_turn_count: row.get::<_, i64>(5)? as usize,
        token_estimate: row.get::<_, i64>(6)? as usize,
        consecutive_failures: row.get::<_, i64>(7)? as usize,
        disabled_until: row.get(8)?,
        last_error: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::session_memory::schema::create_session_memory_tables;
    use std::sync::Mutex;

    #[test]
    fn upserts_and_loads_session_memory() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_session_memory_tables(&conn).unwrap();
        let db = ConversationDb {
            conn: Mutex::new(conn),
        };

        let memory = upsert_memory(
            &db,
            NewSessionMemory {
                session_id: "default".to_string(),
                summary: "goal and constraints".to_string(),
                last_summarized_turn_id: Some("turn_2".to_string()),
                last_summarized_seq: 2,
                checkpoint_id: Some("cp_1".to_string()),
                source_turn_count: 2,
                token_estimate: 64,
            },
        )
        .unwrap();

        let loaded = load_memory(&db, "default").unwrap().unwrap();
        assert_eq!(loaded, memory);
        assert_eq!(loaded.summary, "goal and constraints");
        assert_eq!(loaded.last_summarized_seq, 2);
        assert_eq!(loaded.checkpoint_id.as_deref(), Some("cp_1"));
        assert_eq!(loaded.source_turn_count, 2);
        assert_eq!(loaded.token_estimate, 64);
        assert_eq!(loaded.consecutive_failures, 0);
    }
}

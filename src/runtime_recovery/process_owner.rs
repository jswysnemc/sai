use super::model::{NewRuntimeRecoveryRecord, RuntimeRecoveryKind, RuntimeRecoveryStatus};
use super::repository::record_recovery;
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

struct StaleSubagentProcess {
    id: String,
    last_seq: i64,
}

struct DeadRuntimeProcess {
    id: String,
    pid: i64,
    last_seq: i64,
}

/// 审计 pid 已不存在的运行时进程 owner。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 标记为 stale 的进程数量
pub(crate) fn audit_dead_process_owners(db: &ConversationDb, session_id: &str) -> Result<usize> {
    audit_dead_process_owners_with(db, session_id, process_exists)
}

/// 使用指定存活探测器审计 pid 已不存在的运行时进程 owner。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `exists`: 进程存活探测函数
///
/// 返回:
/// - 标记为 stale 的进程数量
pub(crate) fn audit_dead_process_owners_with(
    db: &ConversationDb,
    session_id: &str,
    exists: impl Fn(u32) -> bool,
) -> Result<usize> {
    let candidates = db.with_conn(|conn| load_running_pid_processes_locked(conn, session_id))?;
    let mut marked = 0;
    for process in candidates {
        let Ok(pid) = u32::try_from(process.pid) else {
            mark_dead_process_stale(db, session_id, &process)?;
            marked += 1;
            continue;
        };
        if !exists(pid) {
            mark_dead_process_stale(db, session_id, &process)?;
            marked += 1;
        }
    }
    Ok(marked)
}

/// 审计已不属于当前进程的子代理运行时 owner。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `current_pid`: 当前 Sai 进程 ID
///
/// 返回:
/// - 标记为 stale 的进程数量
pub(crate) fn audit_stale_subagent_owners(
    db: &ConversationDb,
    session_id: &str,
    current_pid: u32,
) -> Result<usize> {
    let stale =
        db.with_conn(|conn| load_stale_subagent_processes_locked(conn, session_id, current_pid))?;
    for process in &stale {
        mark_subagent_process_stale(db, session_id, process)?;
    }
    Ok(stale.len())
}

/// 读取带 pid 的运行中外部进程。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
///
/// 返回:
/// - 运行中外部进程列表
fn load_running_pid_processes_locked(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<DeadRuntimeProcess>> {
    let mut stmt = conn.prepare(
        "SELECT id, pid, last_seq
         FROM runtime_processes
         WHERE session_id = ?1
         AND status = 'running'
         AND pid IS NOT NULL
         AND process_kind IN ('background_command', 'gateway', 'future_process_spawn')
         ORDER BY updated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![session_id], |row| {
        Ok(DeadRuntimeProcess {
            id: row.get(0)?,
            pid: row.get(1)?,
            last_seq: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// 读取需要标记 stale 的子代理进程。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `current_pid`: 当前 Sai 进程 ID
///
/// 返回:
/// - stale 子代理进程列表
fn load_stale_subagent_processes_locked(
    conn: &Connection,
    session_id: &str,
    current_pid: u32,
) -> Result<Vec<StaleSubagentProcess>> {
    let mut stmt = conn.prepare(
        "SELECT id, last_seq
         FROM runtime_processes
         WHERE session_id = ?1
         AND owner_kind = 'subagent'
         AND process_kind IN ('subagent', 'subagent_task')
         AND status = 'running'
         AND (pid IS NULL OR pid != ?2)
         ORDER BY updated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![session_id, i64::from(current_pid)], |row| {
        Ok(StaleSubagentProcess {
            id: row.get(0)?,
            last_seq: row.get(1)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// 标记单个子代理运行时进程为 stale。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `process`: stale 子代理进程
///
/// 返回:
/// - 标记是否成功
fn mark_subagent_process_stale(
    db: &ConversationDb,
    session_id: &str,
    process: &StaleSubagentProcess,
) -> Result<()> {
    let event_seq = process.last_seq + 1;
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        // 1. 先写 lifecycle 事件，再推进进程状态，确保摘要可以定位最后安全序号
        conn.execute(
            "INSERT OR IGNORE INTO runtime_process_events (
                id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
             ) VALUES (?1, ?2, ?3, 'lifecycle', 'stale_owner', NULL, ?4, ?5)",
            params![
                new_stale_owner_event_id(),
                &process.id,
                event_seq,
                "subagent task owner process is no longer current",
                now,
            ],
        )?;
        conn.execute(
            "UPDATE runtime_processes
             SET status = 'stale',
                 last_seq = MAX(last_seq, ?1),
                 last_seen_at = ?2,
                 updated_at = ?2
             WHERE id = ?3
             AND status = 'running'",
            params![event_seq, now, &process.id],
        )?;
        Ok(())
    })?;
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: Some(process.id.clone()),
            kind: RuntimeRecoveryKind::StaleOwner,
            status: RuntimeRecoveryStatus::Observed,
            reason: "subagent task owner process is no longer current".to_string(),
            last_safe_seq: Some(event_seq),
        },
    )?;
    Ok(())
}

/// 标记已死亡 pid 的运行时进程为 stale。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `process`: 已死亡 pid 的运行时进程
///
/// 返回:
/// - 标记是否成功
fn mark_dead_process_stale(
    db: &ConversationDb,
    session_id: &str,
    process: &DeadRuntimeProcess,
) -> Result<()> {
    let event_seq = process.last_seq + 1;
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        // 1. 记录 stale 生命周期事件，避免摘要继续把死亡 pid 当成 active
        conn.execute(
            "INSERT OR IGNORE INTO runtime_process_events (
                id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
             ) VALUES (?1, ?2, ?3, 'lifecycle', 'stale_owner', NULL, ?4, ?5)",
            params![
                new_stale_owner_event_id(),
                &process.id,
                event_seq,
                format!("runtime process pid {} is no longer alive", process.pid),
                now,
            ],
        )?;
        conn.execute(
            "UPDATE runtime_processes
             SET status = 'stale',
                 last_seq = MAX(last_seq, ?1),
                 last_seen_at = ?2,
                 updated_at = ?2
             WHERE id = ?3
             AND status = 'running'",
            params![event_seq, now, &process.id],
        )?;
        Ok(())
    })?;
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: Some(process.id.clone()),
            kind: RuntimeRecoveryKind::StaleOwner,
            status: RuntimeRecoveryStatus::Observed,
            reason: format!("runtime process pid {} is no longer alive", process.pid),
            last_safe_seq: Some(event_seq),
        },
    )?;
    Ok(())
}

/// 判断进程是否仍存在。
///
/// 参数:
/// - `pid`: 进程 ID
///
/// 返回:
/// - 是否存在
fn process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        if pid == 0 || pid > i32::MAX as u32 {
            return false;
        }
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

/// 创建 stale owner 生命周期事件标识。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 事件标识
fn new_stale_owner_event_id() -> String {
    format!(
        "rpe_stale_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

#[cfg(test)]
mod tests {
    use crate::runtime_recovery::{
        audit_dead_process_owners_with, audit_stale_subagent_owners, record_process,
        session_summary, NewRuntimeProcessRecord, OwnerKind, ProcessKind, RuntimeProcessStatus,
        RuntimeRecoveryKind,
    };
    use crate::state::ConversationDb;

    fn db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    #[test]
    fn audit_marks_subagent_process_from_old_pid_stale() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "subagent_1".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Subagent,
                owner_id: "subagent_1".to_string(),
                process_kind: ProcessKind::Subagent,
                command: "explore".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(100),
                pgid: None,
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();

        let count = audit_stale_subagent_owners(&db, "default", 200).unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();

        assert_eq!(count, 1);
        assert_eq!(summary.active_process_count, 0);
        assert_eq!(summary.stale_process_count, 1);
        assert_eq!(failure.kind, RuntimeRecoveryKind::StaleOwner);
        assert_eq!(failure.process_id.as_deref(), Some("subagent_1"));
    }

    #[test]
    fn audit_marks_dead_background_process_stale() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "background_command_1".to_string(),
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

        let count = audit_dead_process_owners_with(&db, "default", |_| false).unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();

        assert_eq!(count, 1);
        assert_eq!(summary.active_process_count, 0);
        assert_eq!(summary.stale_process_count, 1);
        assert_eq!(failure.kind, RuntimeRecoveryKind::StaleOwner);
        assert_eq!(failure.process_id.as_deref(), Some("background_command_1"));
    }
}

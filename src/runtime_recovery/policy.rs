use super::model::{
    NewRuntimeRecoveryRecord, OwnerKind, RuntimeRecoveryKind, RuntimeRecoveryStatus,
};
use super::repository::record_recovery;
use super::terminator::{PlatformProcessTerminator, ProcessTerminator};
use crate::state::ConversationDb;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};

struct RunningOwnerProcess {
    id: String,
    last_seq: i64,
    pid: Option<i64>,
    pgid: Option<i64>,
}

/// 连接关闭策略执行结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ConnectionClosePolicyOutcome {
    pub terminated: usize,
    pub killed: usize,
    pub detached: usize,
    pub failed: usize,
}

/// 应用命令模式退出时的运行时资源策略。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 标记为 detached 的进程数量
pub(crate) fn apply_command_mode_exit_policy(
    db: &ConversationDb,
    session_id: &str,
) -> Result<usize> {
    detach_running_owner_processes(db, session_id, OwnerKind::CommandMode)
}

/// 使用平台进程终止器应用连接关闭时的运行时资源策略。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `owner_kind`: owner 类型
/// - `owner_id`: owner 标识
///
/// 返回:
/// - 连接关闭策略执行结果
pub(crate) fn apply_connection_close_policy(
    db: &ConversationDb,
    session_id: &str,
    owner_kind: OwnerKind,
    owner_id: &str,
) -> Result<ConnectionClosePolicyOutcome> {
    apply_connection_close_policy_with(
        db,
        session_id,
        owner_kind,
        owner_id,
        &PlatformProcessTerminator,
    )
}

/// 应用连接关闭时的运行时资源策略。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `owner_kind`: owner 类型
/// - `owner_id`: owner 标识
/// - `terminator`: 进程终止器
///
/// 返回:
/// - 连接关闭策略执行结果
#[allow(dead_code)]
pub(crate) fn apply_connection_close_policy_with(
    db: &ConversationDb,
    session_id: &str,
    owner_kind: OwnerKind,
    owner_id: &str,
    terminator: &impl ProcessTerminator,
) -> Result<ConnectionClosePolicyOutcome> {
    let processes = db.with_conn(|conn| {
        load_running_owner_id_processes_locked(conn, session_id, &owner_kind, owner_id)
    })?;
    let mut outcome = ConnectionClosePolicyOutcome::default();
    for process in &processes {
        apply_connection_close_to_process(db, session_id, process, terminator, &mut outcome)?;
    }
    Ok(outcome)
}

/// 将指定 owner 的运行中进程标记为 detached。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `owner_kind`: owner 类型
///
/// 返回:
/// - 标记为 detached 的进程数量
fn detach_running_owner_processes(
    db: &ConversationDb,
    session_id: &str,
    owner_kind: OwnerKind,
) -> Result<usize> {
    let processes =
        db.with_conn(|conn| load_running_owner_processes_locked(conn, session_id, &owner_kind))?;
    for process in &processes {
        detach_process_locked(db, process)?;
    }
    Ok(processes.len())
}

/// 读取指定 owner 的运行中进程。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `owner_kind`: owner 类型
///
/// 返回:
/// - 运行中进程列表
fn load_running_owner_processes_locked(
    conn: &Connection,
    session_id: &str,
    owner_kind: &OwnerKind,
) -> Result<Vec<RunningOwnerProcess>> {
    let mut stmt = conn.prepare(
        "SELECT id, last_seq, pid, pgid
         FROM runtime_processes
         WHERE session_id = ?1
         AND owner_kind = ?2
         AND status = 'running'
         ORDER BY updated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![session_id, owner_kind.as_str()], |row| {
        Ok(RunningOwnerProcess {
            id: row.get(0)?,
            last_seq: row.get(1)?,
            pid: row.get(2)?,
            pgid: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// 读取指定 owner 标识的运行中进程。
///
/// 参数:
/// - `conn`: SQLite 连接
/// - `session_id`: 会话标识
/// - `owner_kind`: owner 类型
/// - `owner_id`: owner 标识
///
/// 返回:
/// - 运行中进程列表
fn load_running_owner_id_processes_locked(
    conn: &Connection,
    session_id: &str,
    owner_kind: &OwnerKind,
    owner_id: &str,
) -> Result<Vec<RunningOwnerProcess>> {
    let mut stmt = conn.prepare(
        "SELECT id, last_seq, pid, pgid
         FROM runtime_processes
         WHERE session_id = ?1
         AND owner_kind = ?2
         AND owner_id = ?3
         AND status = 'running'
         ORDER BY updated_at ASC, id ASC",
    )?;
    let rows = stmt.query_map(params![session_id, owner_kind.as_str(), owner_id], |row| {
        Ok(RunningOwnerProcess {
            id: row.get(0)?,
            last_seq: row.get(1)?,
            pid: row.get(2)?,
            pgid: row.get(3)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// 标记单个运行时进程为 detached。
///
/// 参数:
/// - `db`: 对话数据库
/// - `process`: 运行中进程
///
/// 返回:
/// - 标记是否成功
fn detach_process_locked(db: &ConversationDb, process: &RunningOwnerProcess) -> Result<()> {
    let event_seq = process.last_seq + 1;
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        // 1. 先记录生命周期事件，再更新状态，避免摘要看到没有边界事件的 detached 进程
        conn.execute(
            "INSERT OR IGNORE INTO runtime_process_events (
                id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
             ) VALUES (?1, ?2, ?3, 'lifecycle', 'detached', NULL, ?4, ?5)",
            params![
                new_detached_event_id(),
                &process.id,
                event_seq,
                "command mode ended and detached runtime process",
                now,
            ],
        )?;
        conn.execute(
            "UPDATE runtime_processes
             SET status = 'detached',
                 last_seq = MAX(last_seq, ?1),
                 last_seen_at = ?2,
                 updated_at = ?2
             WHERE id = ?3
             AND status = 'running'",
            params![event_seq, now, &process.id],
        )?;
        Ok(())
    })
}

/// 对单个运行时进程应用连接关闭策略。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `process`: 运行中进程
/// - `terminator`: 进程终止器
/// - `outcome`: 策略执行结果
///
/// 返回:
/// - 策略是否成功应用
fn apply_connection_close_to_process(
    db: &ConversationDb,
    session_id: &str,
    process: &RunningOwnerProcess,
    terminator: &impl ProcessTerminator,
    outcome: &mut ConnectionClosePolicyOutcome,
) -> Result<()> {
    let Some(pid) = valid_pid(process.pid) else {
        write_process_lifecycle_status(
            db,
            process,
            "connection_closed_detached",
            "connection closed and detached pidless runtime process",
            "detached",
        )?;
        outcome.detached += 1;
        return Ok(());
    };
    let pgid = valid_pgid(process.pgid);
    if terminator.terminate(pid, pgid, false)? {
        write_process_lifecycle_status(
            db,
            process,
            "connection_closed_terminated",
            "connection closed and terminated runtime process",
            "stopped",
        )?;
        outcome.terminated += 1;
        return Ok(());
    }
    if terminator.terminate(pid, pgid, true)? {
        write_process_lifecycle_status(
            db,
            process,
            "connection_closed_killed",
            "connection closed and force killed runtime process",
            "stopped",
        )?;
        outcome.killed += 1;
        return Ok(());
    }
    let event_seq = write_process_lifecycle_status(
        db,
        process,
        "connection_close_cleanup_failed",
        "connection closed but runtime process cleanup failed",
        "failed",
    )?;
    outcome.failed += 1;
    record_recovery(
        db,
        NewRuntimeRecoveryRecord {
            session_id: session_id.to_string(),
            process_id: Some(process.id.clone()),
            kind: RuntimeRecoveryKind::DisconnectCleanupFailed,
            status: RuntimeRecoveryStatus::Observed,
            reason: "connection closed but runtime process cleanup failed".to_string(),
            last_safe_seq: Some(event_seq),
        },
    )?;
    Ok(())
}

/// 写入进程生命周期事件并更新状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `process`: 运行中进程
/// - `event_kind`: 生命周期事件类型
/// - `preview`: 生命周期事件摘要
/// - `status`: 新进程状态
///
/// 返回:
/// - 写入的事件序号
fn write_process_lifecycle_status(
    db: &ConversationDb,
    process: &RunningOwnerProcess,
    event_kind: &str,
    preview: &str,
    status: &str,
) -> Result<i64> {
    let event_seq = process.last_seq + 1;
    let now = Utc::now().to_rfc3339();
    db.with_conn(|conn| {
        // 1. 先写生命周期事件，再更新进程状态，保证恢复摘要有明确边界
        conn.execute(
            "INSERT OR IGNORE INTO runtime_process_events (
                id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
             ) VALUES (?1, ?2, ?3, 'lifecycle', ?4, NULL, ?5, ?6)",
            params![
                new_policy_event_id(event_kind),
                &process.id,
                event_seq,
                event_kind,
                preview,
                now,
            ],
        )?;
        conn.execute(
            "UPDATE runtime_processes
             SET status = ?1,
                 last_seq = MAX(last_seq, ?2),
                 last_seen_at = ?3,
                 updated_at = ?3,
                 ended_at = CASE
                    WHEN ?1 IN ('stopped', 'failed') THEN ?3
                    ELSE ended_at
                 END
             WHERE id = ?4
             AND status = 'running'",
            params![status, event_seq, now, &process.id],
        )?;
        Ok(event_seq)
    })
}

/// 转换合法进程 ID。
///
/// 参数:
/// - `pid`: 数据库进程 ID
///
/// 返回:
/// - 合法平台进程 ID
fn valid_pid(pid: Option<i64>) -> Option<u32> {
    let pid = u32::try_from(pid?).ok()?;
    if pid == 0 {
        None
    } else {
        Some(pid)
    }
}

/// 转换合法进程组 ID。
///
/// 参数:
/// - `pgid`: 数据库进程组 ID
///
/// 返回:
/// - 合法平台进程组 ID
fn valid_pgid(pgid: Option<i64>) -> Option<i32> {
    pgid.and_then(|value| i32::try_from(value).ok())
}

/// 创建 detached 生命周期事件标识。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 事件标识
fn new_detached_event_id() -> String {
    format!(
        "rpe_detached_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

/// 创建连接关闭策略事件标识。
///
/// 参数:
/// - `event_kind`: 事件类型
///
/// 返回:
/// - 事件标识
fn new_policy_event_id(event_kind: &str) -> String {
    format!(
        "rpe_{}_{}_{}",
        event_kind,
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

#[cfg(test)]
mod tests {
    use crate::runtime_recovery::{
        apply_command_mode_exit_policy, apply_connection_close_policy_with, record_process,
        session_summary, NewRuntimeProcessRecord, OwnerKind, ProcessKind, ProcessTerminator,
        RuntimeProcessStatus, RuntimeRecoveryKind,
    };
    use crate::state::ConversationDb;
    use anyhow::Result;
    use std::sync::Mutex;

    fn db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    struct FakeTerminator {
        results: Mutex<Vec<bool>>,
        calls: Mutex<Vec<(u32, Option<i32>, bool)>>,
    }

    impl FakeTerminator {
        /// 创建测试终止器。
        ///
        /// 参数:
        /// - `results`: 每次调用返回的停止结果
        ///
        /// 返回:
        /// - 测试终止器
        fn new(results: Vec<bool>) -> Self {
            Self {
                results: Mutex::new(results),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl ProcessTerminator for FakeTerminator {
        fn terminate(&self, pid: u32, pgid: Option<i32>, force: bool) -> Result<bool> {
            self.calls.lock().unwrap().push((pid, pgid, force));
            Ok(self.results.lock().unwrap().remove(0))
        }
    }

    #[test]
    fn command_mode_exit_policy_detaches_running_command_processes() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_command".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::CommandMode,
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

        let detached = apply_command_mode_exit_policy(&db, "default").unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let (status, event_kind): (String, String) = db
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT p.status, e.event_kind
                     FROM runtime_processes p
                     JOIN runtime_process_events e ON e.process_id = p.id
                     WHERE p.id = 'proc_command'
                     AND e.seq = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?)
            })
            .unwrap();

        assert_eq!(detached, 1);
        assert_eq!(status, "detached");
        assert_eq!(event_kind, "detached");
        assert_eq!(summary.active_process_count, 0);
    }

    #[test]
    fn command_mode_exit_policy_keeps_session_processes_running() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "proc_session".to_string(),
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

        let detached = apply_command_mode_exit_policy(&db, "default").unwrap();
        let summary = session_summary(&db, "default").unwrap();

        assert_eq!(detached, 0);
        assert_eq!(summary.active_process_count, 1);
    }

    #[test]
    fn connection_close_policy_kills_gateway_process_after_graceful_failure() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "gateway_proc".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Gateway,
                owner_id: "qq".to_string(),
                process_kind: ProcessKind::Gateway,
                command: "sai gateway qq-bot".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(123),
                pgid: Some(123),
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();
        let terminator = FakeTerminator::new(vec![false, true]);

        let outcome = apply_connection_close_policy_with(
            &db,
            "default",
            OwnerKind::Gateway,
            "qq",
            &terminator,
        )
        .unwrap();
        let (status, event_kind): (String, String) = db
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT p.status, e.event_kind
                     FROM runtime_processes p
                     JOIN runtime_process_events e ON e.process_id = p.id
                     WHERE p.id = 'gateway_proc'
                     AND e.seq = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?)
            })
            .unwrap();

        assert_eq!(outcome.killed, 1);
        assert_eq!(status, "stopped");
        assert_eq!(event_kind, "connection_closed_killed");
        assert_eq!(
            terminator.calls.lock().unwrap().as_slice(),
            &[(123, Some(123), false), (123, Some(123), true)]
        );
    }

    #[test]
    fn connection_close_policy_detaches_pidless_remote_process() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "remote_proc".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::RemoteControl,
                owner_id: "client_1".to_string(),
                process_kind: ProcessKind::FutureProcessSpawn,
                command: "remote process".to_string(),
                cwd: "/tmp".to_string(),
                pid: None,
                pgid: None,
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();
        let terminator = FakeTerminator::new(Vec::new());

        let outcome = apply_connection_close_policy_with(
            &db,
            "default",
            OwnerKind::RemoteControl,
            "client_1",
            &terminator,
        )
        .unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let status: String = db
            .with_conn(|conn| {
                Ok(conn.query_row(
                    "SELECT status FROM runtime_processes WHERE id = 'remote_proc'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .unwrap();

        assert_eq!(outcome.detached, 1);
        assert_eq!(status, "detached");
        assert_eq!(summary.active_process_count, 0);
    }

    #[test]
    fn connection_close_policy_records_cleanup_failure_without_turn() {
        let (_temp, db) = db();
        record_process(
            &db,
            NewRuntimeProcessRecord {
                id: "gateway_proc".to_string(),
                session_id: "default".to_string(),
                owner_kind: OwnerKind::Gateway,
                owner_id: "qq".to_string(),
                process_kind: ProcessKind::Gateway,
                command: "sai gateway qq-bot".to_string(),
                cwd: "/tmp".to_string(),
                pid: Some(123),
                pgid: None,
                status: RuntimeProcessStatus::Running,
                last_seq: 0,
            },
        )
        .unwrap();
        let terminator = FakeTerminator::new(vec![false, false]);

        let outcome = apply_connection_close_policy_with(
            &db,
            "default",
            OwnerKind::Gateway,
            "qq",
            &terminator,
        )
        .unwrap();
        let summary = session_summary(&db, "default").unwrap();
        let failure = summary.latest_failure.unwrap();
        let turn_count: i64 = db
            .with_conn(|conn| {
                Ok(conn.query_row("SELECT COUNT(*) FROM turns", [], |row| row.get(0))?)
            })
            .unwrap();

        assert_eq!(outcome.failed, 1);
        assert_eq!(turn_count, 0);
        assert_eq!(failure.kind, RuntimeRecoveryKind::DisconnectCleanupFailed);
        assert_eq!(failure.process_id.as_deref(), Some("gateway_proc"));
    }
}

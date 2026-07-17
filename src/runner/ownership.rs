use super::SubmissionSource;
use crate::state::ActiveRunSummary;
use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static ACTIVE_RUNS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
const ACTIVE_RUN_LOCK_FILE: &str = "active-run.json";

/// session 运行 owner。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SessionOwner {
    Command,
    Repl,
    Web,
    Gateway,
    ShellIntercept,
}

impl SessionOwner {
    /// 返回 session owner 的稳定文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - owner 文本
    fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Repl => "repl",
            Self::Web => "web",
            Self::Gateway => "gateway",
            Self::ShellIntercept => "shell_intercept",
        }
    }
}

impl From<SubmissionSource> for SessionOwner {
    /// 从 submission 来源转换为 session owner。
    ///
    /// 参数:
    /// - `source`: submission 来源
    ///
    /// 返回:
    /// - session owner
    fn from(source: SubmissionSource) -> Self {
        match source {
            SubmissionSource::Command => Self::Command,
            SubmissionSource::Repl => Self::Repl,
            SubmissionSource::Web => Self::Web,
            SubmissionSource::Gateway => Self::Gateway,
            SubmissionSource::ShellIntercept => Self::ShellIntercept,
        }
    }
}

/// active run 锁文件记录。
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ActiveRunLockRecord {
    session_id: String,
    owner: String,
    pid: u32,
    started_at: String,
}

impl ActiveRunLockRecord {
    /// 构造当前进程的 active run 锁记录。
    ///
    /// 参数:
    /// - `session_id`: 会话 ID
    /// - `owner`: 当前运行 owner
    ///
    /// 返回:
    /// - active run 锁记录
    fn current(session_id: &str, owner: SessionOwner) -> Self {
        Self {
            session_id: session_id.to_string(),
            owner: owner.as_str().to_string(),
            pid: std::process::id(),
            started_at: Utc::now().to_rfc3339(),
        }
    }
}

/// active run guard。
pub(crate) struct ActiveRunGuard {
    registry_key: String,
    session_id: String,
    owner: String,
    lock_path: Option<PathBuf>,
    pid: u32,
    started_at: String,
}

impl ActiveRunGuard {
    /// 获取同一进程内的 active run guard。
    ///
    /// 参数:
    /// - `session_id`: 会话 ID
    /// - `owner`: 当前运行 owner
    ///
    /// 返回:
    /// - active run guard，释放时自动清除占用
    pub(crate) fn acquire(session_id: &str, owner: SessionOwner) -> Result<Self> {
        Self::acquire_inner(session_id, owner, None)
    }

    /// 获取包含跨进程锁文件的 active run guard。
    ///
    /// 参数:
    /// - `session_id`: 会话 ID
    /// - `owner`: 当前运行 owner
    /// - `state_dir`: 当前会话状态目录
    ///
    /// 返回:
    /// - active run guard，释放时自动清除占用和锁文件
    pub(crate) fn acquire_with_state_dir(
        session_id: &str,
        owner: SessionOwner,
        state_dir: &Path,
    ) -> Result<Self> {
        Self::acquire_inner(session_id, owner, Some(state_dir))
    }

    /// 获取 active run guard 的内部实现。
    ///
    /// 参数:
    /// - `session_id`: 会话 ID
    /// - `owner`: 当前运行 owner
    /// - `state_dir`: 当前会话状态目录
    ///
    /// 返回:
    /// - active run guard
    fn acquire_inner(
        session_id: &str,
        owner: SessionOwner,
        state_dir: Option<&Path>,
    ) -> Result<Self> {
        let session_id = session_id.trim();
        if session_id.is_empty() {
            bail!("active run guard requires a session id");
        }
        let record = ActiveRunLockRecord::current(session_id, owner);
        let registry_key = state_dir
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| session_id.to_string());
        insert_process_run(&registry_key, owner)?;
        let lock_path = match state_dir {
            Some(state_dir) => {
                let path = state_dir.join(ACTIVE_RUN_LOCK_FILE);
                if let Err(error) = acquire_durable_lock(&path, &record, owner) {
                    release_process_run(&registry_key);
                    return Err(error);
                }
                Some(path)
            }
            None => None,
        };
        Ok(Self {
            registry_key,
            session_id: session_id.to_string(),
            owner: record.owner,
            lock_path,
            pid: record.pid,
            started_at: record.started_at,
        })
    }

    /// 返回当前 active run 的摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - active run 摘要
    pub(crate) fn summary(&self) -> ActiveRunSummary {
        ActiveRunSummary {
            owner: self.owner.clone(),
            pid: self.pid,
            started_at: self.started_at.clone(),
            lock_path: self
                .lock_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
        }
    }
}

impl Drop for ActiveRunGuard {
    /// 释放 active run guard。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn drop(&mut self) {
        release_process_run(&self.registry_key);
        if let Some(lock_path) = &self.lock_path {
            release_durable_lock(lock_path, &self.session_id, self.pid);
        }
    }
}

/// 写入跨进程 active run 锁文件。
///
/// 参数:
/// - `lock_path`: 锁文件路径
/// - `session_id`: 会话 ID
/// - `owner`: 当前运行 owner
///
/// 返回:
/// - 写入是否成功
fn acquire_durable_lock(
    lock_path: &Path,
    record: &ActiveRunLockRecord,
    owner: SessionOwner,
) -> Result<()> {
    loop {
        match create_lock_file(lock_path, record) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                handle_existing_lock(lock_path, &record.session_id, owner)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// 原子创建锁文件。
///
/// 参数:
/// - `lock_path`: 锁文件路径
/// - `record`: 锁文件记录
///
/// 返回:
/// - 创建是否成功
fn create_lock_file(lock_path: &Path, record: &ActiveRunLockRecord) -> std::io::Result<()> {
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(lock_path)?;
    if let Err(error) = serde_json::to_writer_pretty(file, record) {
        let _ = std::fs::remove_file(lock_path);
        return Err(std::io::Error::new(ErrorKind::InvalidData, error));
    }
    Ok(())
}

/// 处理已存在的锁文件。
///
/// 参数:
/// - `lock_path`: 锁文件路径
/// - `session_id`: 会话 ID
/// - `owner`: 当前运行 owner
///
/// 返回:
/// - 处理是否成功
fn handle_existing_lock(lock_path: &Path, session_id: &str, owner: SessionOwner) -> Result<()> {
    match read_lock_record(lock_path) {
        Some(record) if record.session_id == session_id && process_exists(record.pid) => {
            bail!(
                "session {session_id} is already running for {} in process {}",
                record.owner,
                record.pid
            );
        }
        _ => remove_stale_lock(lock_path, session_id, owner),
    }
}

/// 读取锁文件记录。
///
/// 参数:
/// - `lock_path`: 锁文件路径
///
/// 返回:
/// - 可用锁记录
fn read_lock_record(lock_path: &Path) -> Option<ActiveRunLockRecord> {
    let content = std::fs::read_to_string(lock_path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 删除 stale 锁文件。
///
/// 参数:
/// - `lock_path`: 锁文件路径
/// - `session_id`: 会话 ID
/// - `owner`: 当前运行 owner
///
/// 返回:
/// - 删除是否成功
fn remove_stale_lock(lock_path: &Path, session_id: &str, owner: SessionOwner) -> Result<()> {
    match std::fs::remove_file(lock_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).map_err(|error| {
            anyhow::anyhow!(
                "failed to recover stale active run lock for session {session_id} and {:?}: {error}",
                owner
            )
        }),
    }
}

/// 释放跨进程 active run 锁文件。
///
/// 参数:
/// - `lock_path`: 锁文件路径
/// - `session_id`: 会话 ID
/// - `pid`: 当前进程 ID
///
/// 返回:
/// - 无
fn release_durable_lock(lock_path: &Path, session_id: &str, pid: u32) {
    if let Some(record) = read_lock_record(lock_path) {
        if record.session_id == session_id && record.pid == pid {
            let _ = std::fs::remove_file(lock_path);
        }
    }
}

/// 注册进程内 active run。
///
/// 参数:
/// - `session_id`: 会话 ID
/// - `owner`: 当前运行 owner
///
/// 返回:
/// - 注册是否成功
fn insert_process_run(session_id: &str, owner: SessionOwner) -> Result<()> {
    let active_runs = active_runs();
    let mut runs = active_runs
        .lock()
        .map_err(|_| anyhow::anyhow!("active run registry is poisoned"))?;
    if !runs.insert(session_id.to_string()) {
        bail!(
            "session {session_id} is already running in this process for {:?}",
            owner
        );
    }
    Ok(())
}

/// 释放进程内 active run。
///
/// 参数:
/// - `session_id`: 会话 ID
///
/// 返回:
/// - 无
fn release_process_run(session_id: &str) {
    if let Ok(mut runs) = active_runs().lock() {
        runs.remove(session_id);
    }
}

/// 判断进程是否仍存在。
///
/// 参数:
/// - `pid`: 进程 ID
///
/// 返回:
/// - 是否存在
fn process_exists(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        if pid > i32::MAX as u32 {
            return false;
        }
        let status = unsafe { libc::kill(pid as i32, 0) };
        status == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
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

/// 返回 active run 全局注册表。
///
/// 参数:
/// - 无
///
/// 返回:
/// - active run 全局注册表
fn active_runs() -> &'static Mutex<HashSet<String>> {
    ACTIVE_RUNS.get_or_init(|| Mutex::new(HashSet::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成唯一测试 session ID。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - session ID
    fn unique_session_id() -> String {
        format!(
            "test-session-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )
    }

    /// 验证同一 session 不能被两个 owner 同时占用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn active_run_guard_rejects_second_owner() {
        let session_id = unique_session_id();
        let first = ActiveRunGuard::acquire(&session_id, SessionOwner::Command).unwrap();

        let second = ActiveRunGuard::acquire(&session_id, SessionOwner::Gateway);

        assert!(second.is_err());
        drop(first);
        assert!(ActiveRunGuard::acquire(&session_id, SessionOwner::Gateway).is_ok());
    }

    /// 验证跨进程锁会拒绝仍存活的 pid。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn durable_lock_rejects_live_pid() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();
        let lock_path = temp.path().join(ACTIVE_RUN_LOCK_FILE);
        let record = ActiveRunLockRecord {
            session_id: session_id.clone(),
            owner: "command".to_string(),
            pid: std::process::id(),
            started_at: Utc::now().to_rfc3339(),
        };
        std::fs::write(&lock_path, serde_json::to_string_pretty(&record).unwrap()).unwrap();

        let guard =
            ActiveRunGuard::acquire_with_state_dir(&session_id, SessionOwner::Gateway, temp.path());

        assert!(guard.is_err());
        assert!(ActiveRunGuard::acquire(&session_id, SessionOwner::Gateway).is_ok());
    }

    /// 验证 stale pid 锁会被恢复并替换。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn durable_lock_recovers_stale_pid() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();
        let lock_path = temp.path().join(ACTIVE_RUN_LOCK_FILE);
        let record = ActiveRunLockRecord {
            session_id: session_id.clone(),
            owner: "command".to_string(),
            pid: u32::MAX,
            started_at: Utc::now().to_rfc3339(),
        };
        std::fs::write(&lock_path, serde_json::to_string_pretty(&record).unwrap()).unwrap();

        let guard =
            ActiveRunGuard::acquire_with_state_dir(&session_id, SessionOwner::Gateway, temp.path())
                .unwrap();
        let replaced = read_lock_record(&lock_path).unwrap();

        assert_eq!(replaced.session_id, session_id);
        assert_eq!(replaced.owner, "gateway");
        assert_eq!(replaced.pid, std::process::id());
        drop(guard);
        assert!(!lock_path.exists());
    }

    /// 验证释放 guard 会删除匹配的跨进程锁。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn durable_lock_is_removed_on_drop() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();
        let lock_path = temp.path().join(ACTIVE_RUN_LOCK_FILE);

        let guard =
            ActiveRunGuard::acquire_with_state_dir(&session_id, SessionOwner::Command, temp.path())
                .unwrap();

        assert!(lock_path.exists());
        drop(guard);
        assert!(!lock_path.exists());
    }

    /// 验证不同工作区中同名会话可以并行获取运行所有权。
    #[test]
    fn same_session_id_in_different_state_directories_can_run_in_parallel() {
        let first_dir = tempfile::tempdir().unwrap();
        let second_dir = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();
        let first = ActiveRunGuard::acquire_with_state_dir(
            &session_id,
            SessionOwner::Web,
            first_dir.path(),
        )
        .unwrap();
        let second = ActiveRunGuard::acquire_with_state_dir(
            &session_id,
            SessionOwner::Web,
            second_dir.path(),
        );

        assert!(second.is_ok());
        drop(first);
    }
}

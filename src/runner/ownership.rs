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
use std::time::Duration;

static ACTIVE_RUNS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
const ACTIVE_RUN_LOCK_FILE: &str = "active-run.json";

/// 会话持有者登记文件。
///
/// 与 `active-run.json` 职责分离：后者是**轮次级**互斥（一轮跑完即释放），
/// 前者是**会话级**长期登记（TUI 或 Web 打开会话期间一直存在），
/// 供其它进程发现"谁在持有这个会话"并连接它的事件流。
const SESSION_HOLDER_FILE: &str = "session-holder.json";

/// 会话持有者登记表格式版本。
const HOLDER_SCHEMA: u8 = 1;

/// 持有者心跳写入间隔。
pub(crate) const HOLDER_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(2);

/// 心跳判死阈值：超过这么久没更新就认为持有者已失联。
const HOLDER_HEARTBEAT_STALE: Duration = Duration::from_secs(15);

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
pub(crate) struct ActiveRunLockRecord {
    pub(crate) session_id: String,
    pub(crate) owner: String,
    pub(crate) pid: u32,
    pub(crate) started_at: String,
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

/// 会话持有者暴露事件流的端点类型。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum TransportKind {
    /// Unix domain socket（Linux / macOS）
    Unix,
    /// Windows named pipe
    WinPipe,
}

/// 会话持有者的事件流端点，供观察者连接。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct TransportRef {
    pub(crate) kind: TransportKind,
    /// socket 路径或 named pipe 名称
    pub(crate) path: String,
}

/// 会话持有者登记表。
///
/// 新增字段一律带 `#[serde(default)]`：读到旧格式时按缺省值处理，
/// 观察者据此降级为只读，而不是直接报错。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionHolderRecord {
    pub(crate) schema: u8,
    pub(crate) session_id: String,
    pub(crate) owner: String,
    pub(crate) pid: u32,
    pub(crate) started_at: String,
    #[serde(default)]
    pub(crate) heartbeat_at: Option<String>,
    #[serde(default)]
    pub(crate) transport: Option<TransportRef>,
    #[serde(default)]
    pub(crate) watchers: u32,
}

impl SessionHolderRecord {
    /// 构造当前进程的持有者登记。
    fn current(session_id: &str, owner: SessionOwner) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            schema: HOLDER_SCHEMA,
            session_id: session_id.to_string(),
            owner: owner.as_str().to_string(),
            pid: std::process::id(),
            started_at: now.clone(),
            heartbeat_at: Some(now),
            transport: None,
            watchers: 0,
        }
    }
}

/// 会话持有者守卫；`Drop` 时撤销登记。
pub(crate) struct SessionHolderGuard {
    path: PathBuf,
    session_id: String,
    pid: u32,
}

impl SessionHolderGuard {
    /// 登记为会话持有者。
    ///
    /// 语义互斥：已有存活持有者时返回错误，调用方应降级为观察者。
    ///
    /// 参数:
    /// - `state_dir`: 会话状态目录
    /// - `session_id`: 会话 ID
    /// - `owner`: 持有者类型
    ///
    /// 返回:
    /// - 持有者守卫，释放时撤销登记
    pub(crate) fn acquire(state_dir: &Path, session_id: &str, owner: SessionOwner) -> Result<Self> {
        let path = state_dir.join(SESSION_HOLDER_FILE);
        if let Some(existing) = read_holder_record(&path) {
            if existing.session_id == session_id && holder_is_alive(&existing) {
                bail!(
                    "session {session_id} is already held by {} in process {}",
                    existing.owner,
                    existing.pid
                );
            }
            let _ = std::fs::remove_file(&path);
        }
        let record = SessionHolderRecord::current(session_id, owner);
        write_holder_record(&path, &record)?;
        Ok(Self {
            path,
            session_id: session_id.to_string(),
            pid: record.pid,
        })
    }

    /// 更新心跳时间戳。
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn heartbeat(&self) -> Result<()> {
        let Some(mut record) = read_holder_record(&self.path) else {
            return Ok(());
        };
        record.heartbeat_at = Some(Utc::now().to_rfc3339());
        write_holder_record(&self.path, &record)
    }

    /// 登记事件流端点，供观察者连接。
    ///
    /// 参数:
    /// - `transport`: 端点描述
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn publish_transport(&self, transport: TransportRef) -> Result<()> {
        let Some(mut record) = read_holder_record(&self.path) else {
            return Ok(());
        };
        record.transport = Some(transport);
        write_holder_record(&self.path, &record)
    }

    /// 更新观察者数量（仅用于诊断展示）。
    ///
    /// 参数:
    /// - `watchers`: 当前观察者数
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn set_watchers(&self, watchers: u32) -> Result<()> {
        let Some(mut record) = read_holder_record(&self.path) else {
            return Ok(());
        };
        record.watchers = watchers;
        write_holder_record(&self.path, &record)
    }
}

impl Drop for SessionHolderGuard {
    /// 撤销本进程自己的登记。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn drop(&mut self) {
        if let Some(record) = read_holder_record(&self.path) {
            if record.session_id == self.session_id && record.pid == self.pid {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// 读取当前会话持有者登记。
///
/// 只读取、不创建任何文件；观察者用它发现持有者并连接其事件流。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 持有者登记；无登记或内容不可解析时为空
pub(crate) fn session_holder(state_dir: &Path) -> Option<SessionHolderRecord> {
    read_holder_record(&state_dir.join(SESSION_HOLDER_FILE))
}

/// 读取会话当前正在跑的那一轮（只读，不创建也不清理锁文件）。
///
/// 崩溃残留的锁文件会让会话永远显示"正在跑一轮"，因此这里按 pid 判活，
/// 进程已退出的锁直接视为无人运行。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 轮次锁记录；无锁文件或持有进程已退出时为空
pub(crate) fn active_run(state_dir: &Path) -> Option<ActiveRunLockRecord> {
    let record = read_lock_record(&state_dir.join(ACTIVE_RUN_LOCK_FILE))?;
    process_exists(record.pid).then_some(record)
}

/// 判断持有者是否仍然存活。
///
/// 先判心跳：心跳新鲜就直接判定存活，避免每次都去查进程——
/// Windows 的进程检查要起 `tasklist` 子进程，很贵。
/// 心跳过期后才回落到进程存在性检查。
///
/// 参数:
/// - `record`: 持有者登记
///
/// 返回:
/// - 持有者存活为 true
pub(crate) fn holder_is_alive(record: &SessionHolderRecord) -> bool {
    let Some(heartbeat_at) = record.heartbeat_at.as_deref() else {
        // 旧格式没有心跳字段，直接查进程
        return process_exists(record.pid);
    };
    let Ok(heartbeat) = chrono::DateTime::parse_from_rfc3339(heartbeat_at) else {
        return process_exists(record.pid);
    };
    let age = Utc::now().signed_duration_since(heartbeat.with_timezone(&Utc));
    let stale = chrono::Duration::seconds(HOLDER_HEARTBEAT_STALE.as_secs() as i64);
    if age <= stale {
        return true;
    }
    process_exists(record.pid)
}

/// 读取持有者登记文件。
fn read_holder_record(path: &Path) -> Option<SessionHolderRecord> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// 原子写入持有者登记文件（tmp + rename）。
fn write_holder_record(path: &Path, record: &SessionHolderRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    serde_json::to_writer_pretty(file, record)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
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
    // 重试上限 + 退避：清理 stale 锁后立刻重建仍可能撞上另一个正在竞争的
    // 进程，无退避的空转会把 CPU 打满
    const MAX_ATTEMPTS: u32 = 50;
    for attempt in 0..MAX_ATTEMPTS {
        match create_lock_file(lock_path, record) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                handle_existing_lock(lock_path, &record.session_id, owner)?;
            }
            Err(error) => return Err(error.into()),
        }
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    bail!(
        "could not acquire the active run lock for session {} after {MAX_ATTEMPTS} attempts",
        record.session_id
    )
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

    /// 持有者登记后可被读到，释放后登记撤销。
    #[test]
    fn session_holder_registration_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();

        let guard =
            SessionHolderGuard::acquire(temp.path(), &session_id, SessionOwner::Repl).unwrap();
        let holder = session_holder(temp.path()).unwrap();

        assert_eq!(holder.session_id, session_id);
        assert_eq!(holder.owner, "repl");
        assert_eq!(holder.pid, std::process::id());
        assert!(holder.heartbeat_at.is_some());
        assert!(holder.transport.is_none());
        drop(guard);
        assert!(session_holder(temp.path()).is_none());
    }

    /// 已有存活持有者时，第二个进程只能降级为观察者。
    #[test]
    fn session_holder_rejects_a_second_live_holder() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();

        let first =
            SessionHolderGuard::acquire(temp.path(), &session_id, SessionOwner::Repl).unwrap();
        let second = SessionHolderGuard::acquire(temp.path(), &session_id, SessionOwner::Web);

        assert!(second.is_err(), "a live holder must not be displaced");
        assert_eq!(session_holder(temp.path()).unwrap().owner, "repl");
        drop(first);
    }

    /// 心跳过期且进程已死时，持有者被判定为失联，登记可被接管。
    #[test]
    fn stale_session_holder_can_be_taken_over() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();
        let path = temp.path().join(SESSION_HOLDER_FILE);
        // 心跳必须过期：新鲜心跳会被直接判活，不该被接管
        let long_past =
            Utc::now() - chrono::Duration::seconds(HOLDER_HEARTBEAT_STALE.as_secs() as i64 + 60);
        let stale = SessionHolderRecord {
            schema: HOLDER_SCHEMA,
            session_id: session_id.clone(),
            owner: "repl".to_string(),
            pid: u32::MAX,
            started_at: long_past.to_rfc3339(),
            heartbeat_at: Some(long_past.to_rfc3339()),
            transport: None,
            watchers: 0,
        };
        write_holder_record(&path, &stale).unwrap();

        let taken =
            SessionHolderGuard::acquire(temp.path(), &session_id, SessionOwner::Web).unwrap();

        assert_eq!(session_holder(temp.path()).unwrap().owner, "web");
        drop(taken);
    }

    /// 心跳新鲜时直接判活，不去查进程——Windows 的进程检查很贵。
    #[test]
    fn fresh_heartbeat_keeps_a_dead_pid_alive() {
        let record = SessionHolderRecord {
            schema: HOLDER_SCHEMA,
            session_id: "session".to_string(),
            owner: "web".to_string(),
            pid: u32::MAX,
            started_at: Utc::now().to_rfc3339(),
            heartbeat_at: Some(Utc::now().to_rfc3339()),
            transport: None,
            watchers: 0,
        };

        assert!(holder_is_alive(&record));
    }

    /// 心跳过期后回落到进程检查，pid 不存在则判定失联。
    #[test]
    fn expired_heartbeat_falls_back_to_process_check() {
        let long_past = Utc::now() - chrono::Duration::seconds(HOLDER_HEARTBEAT_STALE.as_secs() as i64 + 60);
        let record = SessionHolderRecord {
            schema: HOLDER_SCHEMA,
            session_id: "session".to_string(),
            owner: "web".to_string(),
            pid: u32::MAX,
            started_at: long_past.to_rfc3339(),
            heartbeat_at: Some(long_past.to_rfc3339()),
            transport: None,
            watchers: 0,
        };

        assert!(!holder_is_alive(&record));
    }

    /// 旧格式没有心跳字段时按缺失处理，不报错。
    #[test]
    fn legacy_holder_record_without_heartbeat_is_readable() {
        let record = SessionHolderRecord {
            schema: HOLDER_SCHEMA,
            session_id: "session".to_string(),
            owner: "web".to_string(),
            pid: u32::MAX,
            started_at: Utc::now().to_rfc3339(),
            heartbeat_at: None,
            transport: None,
            watchers: 0,
        };
        let json = serde_json::to_string(&record).unwrap();
        // 去掉可选字段，模拟旧版本写入的内容
        let legacy = json
            .replace(",\"heartbeat_at\":null", "")
            .replace(",\"transport\":null", "")
            .replace(",\"watchers\":0", "");

        let parsed: SessionHolderRecord = serde_json::from_str(&legacy).unwrap();

        assert!(parsed.heartbeat_at.is_none());
        assert!(parsed.transport.is_none());
        assert_eq!(parsed.watchers, 0);
    }

    /// 观察者连接所需的端点可以登记并被读到。
    #[test]
    fn transport_can_be_published_for_observers() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = unique_session_id();
        let guard =
            SessionHolderGuard::acquire(temp.path(), &session_id, SessionOwner::Web).unwrap();

        guard
            .publish_transport(TransportRef {
                kind: TransportKind::Unix,
                path: "/tmp/sai-bus-abc12345".to_string(),
            })
            .unwrap();

        let holder = session_holder(temp.path()).unwrap();
        let transport = holder.transport.unwrap();
        assert_eq!(transport.kind, TransportKind::Unix);
        assert_eq!(transport.path, "/tmp/sai-bus-abc12345");
        drop(guard);
    }
}

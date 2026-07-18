use super::background_runtime::{
    background_runtime_process_id, record_runtime_output_read, sync_runtime_task,
    sync_runtime_tasks, LogTail,
};
use super::background_timeout::{is_unlimited, timeout_seconds_from_args};
use super::process::{process_exists, spawn_background_shell, terminate_process};
use super::store::{unix_seconds, BackgroundCommandStore, BackgroundCommandTask};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::runtime_recovery::{OwnerKind, ProcessKind};
use crate::state::StateStore;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 后台任务运行时 owner 元数据。
#[derive(Debug, Clone)]
pub(super) struct BackgroundRuntimeOwner {
    pub(super) owner_kind: OwnerKind,
    pub(super) owner_id: String,
    pub(super) process_kind: ProcessKind,
}

impl BackgroundRuntimeOwner {
    /// 创建交互式会话运行时 owner。
    ///
    /// 参数:
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 后台任务运行时 owner 元数据
    pub(super) fn session(session_id: &str) -> Self {
        Self {
            owner_kind: OwnerKind::Session,
            owner_id: session_id.to_string(),
            process_kind: ProcessKind::BackgroundCommand,
        }
    }

    /// 创建命令模式运行时 owner。
    ///
    /// 参数:
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 后台任务运行时 owner 元数据
    pub(super) fn command_mode(session_id: &str) -> Self {
        Self {
            owner_kind: OwnerKind::CommandMode,
            owner_id: session_id.to_string(),
            process_kind: ProcessKind::BackgroundCommand,
        }
    }
}

/// 启动后台命令。
///
/// 参数:
/// - `args`: 工具参数
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `allowed`: 是否允许命令执行
/// - `runtime_owner`: 可选运行时 owner 元数据
///
/// 返回:
/// - JSON 格式任务信息
pub(super) fn start_background_task(
    args: Value,
    config: &AppConfig,
    paths: &SaiPaths,
    allowed: bool,
    runtime_owner: Option<BackgroundRuntimeOwner>,
) -> Result<String> {
    if !allowed {
        bail!("{}", t("command execution is disabled; set skills.allow_command_execution=true in config.jsonc to enable background commands", "命令执行已禁用；请在 config.jsonc 中设置 skills.allow_command_execution=true 以启用后台命令"));
    }
    if !config.tools.background_commands_enabled {
        bail!(
            "{}",
            t("background commands are disabled", "后台命令已禁用")
        );
    }
    let command = required(&args, "command")?;
    let cwd = args
        .get("cwd")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(expand_path)
        .unwrap_or(crate::runtime_cwd::current_dir()?);
    if !cwd.is_dir() {
        bail!(
            "{}: {}",
            t(
                "background command cwd is not a directory",
                "后台命令工作目录不是目录"
            ),
            cwd.display()
        );
    }
    let label = args
        .get("label")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("background command")
        .to_string();
    let timeout_seconds =
        timeout_seconds_from_args(&args, config.tools.background_command_timeout_seconds);
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    store.init()?;
    let state = state_for_runtime_owner(paths, runtime_owner.as_ref())?;
    let goal_id = match runtime_owner.as_ref() {
        Some(owner) if owner.owner_kind == OwnerKind::Session => state
            .goal()?
            .filter(|goal| goal.status.accepts_external_wake())
            .map(|goal| goal.id),
        _ => None,
    };
    let now = unix_seconds();
    let id_prefix = sanitize_id(&label);
    let stdout_log = store.logs_dir().join(format!("{now}-{id_prefix}.out.log"));
    let stderr_log = store.logs_dir().join(format!("{now}-{id_prefix}.err.log"));
    let stdout = std::fs::File::create(&stdout_log)?;
    let stderr = std::fs::File::create(&stderr_log)?;
    let process =
        spawn_background_shell(&command, &cwd, &config.tools.command_shell, stdout, stderr)?;
    let task_id = format!("{now}-{}", process.pid);
    let runtime_process_id = background_runtime_process_id(&task_id);
    let task = BackgroundCommandTask {
        id: task_id.clone(),
        runtime_process_id: Some(runtime_process_id),
        runtime_owner_kind: runtime_owner
            .as_ref()
            .map(|owner| owner.owner_kind.as_str().to_string()),
        runtime_owner_id: runtime_owner.as_ref().map(|owner| owner.owner_id.clone()),
        runtime_process_kind: runtime_owner
            .as_ref()
            .map(|owner| owner.process_kind.as_str().to_string()),
        goal_id,
        label,
        command,
        cwd: cwd.display().to_string(),
        pid: process.pid,
        pgid: process.pgid,
        status: "running".to_string(),
        stdout_log: stdout_log.display().to_string(),
        stderr_log: stderr_log.display().to_string(),
        started_at: now,
        updated_at: now,
        timeout_seconds,
        completion_notified: false,
    };
    store.upsert(task.clone())?;
    sync_runtime_task(&state, &task)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "task": task,
        "note": "Use background_command with action=list, action=output, action=stop, or action=cleanup to manage this task."
    }))?)
}

/// 列出后台命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - JSON 格式任务列表
pub(super) async fn list_background_tasks(paths: &SaiPaths, config: &AppConfig) -> Result<String> {
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut tasks = store.load()?;
    refresh_task_statuses(&mut tasks, config).await;
    store.save(&tasks)?;
    let state = StateStore::new(paths)?;
    sync_runtime_tasks(&state, &tasks)?;
    // 1. 网关进程由网关管理页独立管理，通用后台任务列表不展示
    tasks.retain(|task| !is_gateway_owned_task(task));
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "tasks": tasks,
    }))?)
}

/// 判断后台任务是否属于网关进程。
///
/// 参数:
/// - `task`: 后台任务
///
/// 返回:
/// - 属于网关时返回 true
fn is_gateway_owned_task(task: &BackgroundCommandTask) -> bool {
    task.runtime_owner_kind.as_deref() == Some(OwnerKind::Gateway.as_str())
        || task.label.starts_with("gateway:")
}

/// 读取后台命令输出。
///
/// 参数:
/// - `args`: 工具参数
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 格式输出
pub(super) async fn read_background_task_output(
    args: Value,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<String> {
    let task_id = required(&args, "task_id")?;
    let stream = args
        .get("stream")
        .and_then(Value::as_str)
        .unwrap_or("all")
        .trim();
    let tail_lines = args
        .get("tail_lines")
        .and_then(Value::as_u64)
        .unwrap_or(200)
        .clamp(1, 2000) as usize;
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut tasks = store.load()?;
    refresh_task_statuses(&mut tasks, config).await;
    store.save(&tasks)?;
    let state = StateStore::new(paths)?;
    sync_runtime_tasks(&state, &tasks)?;
    let task = find_task(&tasks, &task_id)?;
    let max_bytes = config.tools.background_command_log_max_bytes;
    let stdout = if matches!(stream, "stdout" | "all") {
        Some(read_log_tail(&task.stdout_log, tail_lines, max_bytes)?)
    } else {
        None
    };
    let stderr = if matches!(stream, "stderr" | "all") {
        Some(read_log_tail(&task.stderr_log, tail_lines, max_bytes)?)
    } else {
        None
    };
    if let Some(output) = stdout.as_ref() {
        record_runtime_output_read(&state, task, "stdout", &task.stdout_log, output)?;
    }
    if let Some(output) = stderr.as_ref() {
        record_runtime_output_read(&state, task, "stderr", &task.stderr_log, output)?;
    }
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "task": task,
        "stdout": stdout.as_ref().map(|output| output.text.clone()),
        "stderr": stderr.as_ref().map(|output| output.text.clone()),
        "stdout_truncated": stdout.as_ref().map(|output| output.truncated).unwrap_or(false),
        "stderr_truncated": stderr.as_ref().map(|output| output.truncated).unwrap_or(false),
        "log_max_bytes": max_bytes,
        "tail_lines": tail_lines,
    }))?)
}

/// 停止后台命令。
///
/// 参数:
/// - `args`: 工具参数
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 格式停止结果
pub(super) async fn stop_background_task(
    args: Value,
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<String> {
    let task_id = required(&args, "task_id")?;
    let force = args.get("force").and_then(Value::as_bool).unwrap_or(false);
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut tasks = store.load()?;
    refresh_task_statuses(&mut tasks, config).await;
    let task = tasks
        .iter_mut()
        .find(|item| item.id == task_id)
        .ok_or_else(|| anyhow::anyhow!("background command not found: {task_id}"))?;
    let was_running = task.status == "running" && process_exists(task.pid);
    if was_running {
        terminate_process(task.pid, task.pgid, force).await;
        if !force {
            tokio::time::sleep(Duration::from_secs(
                config.tools.background_command_stop_grace_seconds,
            ))
            .await;
            if process_exists(task.pid) {
                terminate_process(task.pid, task.pgid, true).await;
            }
        }
        task.status = "stopped".to_string();
        task.updated_at = unix_seconds();
    }
    let task = task.clone();
    store.save(&tasks)?;
    let state = StateStore::new(paths)?;
    sync_runtime_tasks(&state, &tasks)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "was_running": was_running,
        "task": task,
    }))?)
}

/// 清理已结束后台命令记录。
///
/// 参数:
/// - `args`: 工具参数
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - JSON 格式清理结果
pub(super) async fn cleanup_background_tasks(
    args: Value,
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<String> {
    let remove_logs = args
        .get("remove_logs")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let mut tasks = store.load()?;
    refresh_task_statuses(&mut tasks, config).await;
    let state = StateStore::new(paths)?;
    sync_runtime_tasks(&state, &tasks)?;
    let mut removed = Vec::new();
    tasks.retain(|task| {
        if task.status == "running" {
            return true;
        }
        if remove_logs {
            let _ = std::fs::remove_file(&task.stdout_log);
            let _ = std::fs::remove_file(&task.stderr_log);
        }
        removed.push(task.id.clone());
        false
    });
    store.save(&tasks)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "removed": removed,
        "remaining": tasks.len(),
    }))?)
}

/// 刷新任务运行状态。
///
/// 参数:
/// - `tasks`: 任务列表
/// - `config`: 应用配置
pub(super) async fn refresh_task_statuses(
    tasks: &mut [BackgroundCommandTask],
    config: &AppConfig,
) -> bool {
    let now = unix_seconds();
    let mut changed = false;
    for task in tasks {
        if task.status != "running" {
            continue;
        }
        if !process_exists(task.pid) {
            task.status = "exited".to_string();
            task.updated_at = now;
            changed = true;
            continue;
        }
        if !is_unlimited(task.timeout_seconds)
            && now.saturating_sub(task.started_at) >= task.timeout_seconds
        {
            terminate_process(task.pid, task.pgid, false).await;
            tokio::time::sleep(Duration::from_secs(
                config.tools.background_command_stop_grace_seconds,
            ))
            .await;
            if process_exists(task.pid) {
                terminate_process(task.pid, task.pgid, true).await;
            }
            task.status = "timed_out".to_string();
            task.updated_at = unix_seconds();
            changed = true;
        }
    }
    changed
}

/// 打开后台任务所属会话的状态存储。
fn state_for_runtime_owner(
    paths: &SaiPaths,
    owner: Option<&BackgroundRuntimeOwner>,
) -> Result<StateStore> {
    match owner {
        Some(owner) if owner.owner_kind == OwnerKind::Session => {
            StateStore::for_session(paths, &owner.owner_id)
        }
        Some(owner) if owner.owner_kind == OwnerKind::CommandMode => {
            StateStore::for_session(paths, &owner.owner_id).or_else(|_| StateStore::new(paths))
        }
        _ => StateStore::new(paths),
    }
}

/// 查找后台任务。
///
/// 参数:
/// - `tasks`: 任务列表
/// - `task_id`: 任务 ID
///
/// 返回:
/// - 匹配任务
fn find_task<'a>(
    tasks: &'a [BackgroundCommandTask],
    task_id: &str,
) -> Result<&'a BackgroundCommandTask> {
    tasks
        .iter()
        .find(|item| item.id == task_id)
        .ok_or_else(|| anyhow::anyhow!("background command not found: {task_id}"))
}

/// 读取日志末尾若干行。
///
/// 参数:
/// - `path`: 日志路径
/// - `tail_lines`: 末尾行数
/// - `max_bytes`: 最大读取字节数
///
/// 返回:
/// - 日志文本
pub(super) fn read_log_tail(path: &str, tail_lines: usize, max_bytes: u64) -> Result<LogTail> {
    let path = Path::new(path);
    if !path.exists() {
        return Ok(LogTail::empty(max_bytes.max(1)));
    }
    let metadata = std::fs::metadata(path)?;
    let max_bytes = max_bytes.max(1);
    let start = metadata.len().saturating_sub(max_bytes);
    let mut file = std::fs::File::open(path)?;
    use std::io::{Read, Seek, SeekFrom};
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(tail_lines);
    Ok(LogTail::new(
        lines[start..].join("\n"),
        metadata.len() > max_bytes,
        metadata.len(),
        bytes.len() as u64,
        max_bytes,
    ))
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 参数值
fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    } else {
        Ok(value.to_string())
    }
}

/// 展开路径。
///
/// 参数:
/// - `value`: 原始路径
///
/// 返回:
/// - 展开后的路径
fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

/// 生成适合任务 ID 和日志名的短标签。
///
/// 参数:
/// - `value`: 原始标签
///
/// 返回:
/// - 安全标签
fn sanitize_id(value: &str) -> String {
    let mut output = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if matches!(ch, '-' | '_' | '.') {
                Some(ch)
            } else if ch.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .take(40)
        .collect::<String>();
    if output.is_empty() {
        output = "command".to_string();
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StateStore;
    use std::path::PathBuf;

    fn test_paths(state_dir: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: PathBuf::new(),
            config_file: PathBuf::new(),
            secrets_file: PathBuf::new(),
            skills_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            state_dir,
            pictures_dir: PathBuf::new(),
            fish_hook_file: PathBuf::new(),
            bash_hook_file: PathBuf::new(),
            zsh_hook_file: PathBuf::new(),
            powershell_hook_file: PathBuf::new(),
        }
    }

    #[test]
    fn sanitize_id_keeps_safe_subset() {
        assert_eq!(sanitize_id("Dev Server 01!"), "dev-server-01");
    }

    /// 验证交互会话 owner 定位失败时不会退回当前活动会话。
    #[test]
    fn session_owner_requires_exact_session_state() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let owner = BackgroundRuntimeOwner::session("missing-session");

        assert!(state_for_runtime_owner(&paths, Some(&owner)).is_err());
    }

    #[test]
    fn gateway_owned_tasks_are_detected_by_owner_and_label() {
        let mut task = BackgroundCommandTask {
            id: "1".to_string(),
            runtime_process_id: None,
            runtime_owner_kind: Some("gateway".to_string()),
            runtime_owner_id: Some("qq".to_string()),
            runtime_process_kind: Some("gateway".to_string()),
            goal_id: None,
            label: "gateway:qq".to_string(),
            command: "sai gateway qq-bot".to_string(),
            cwd: ".".to_string(),
            pid: 100,
            pgid: None,
            status: "running".to_string(),
            stdout_log: "stdout.log".to_string(),
            stderr_log: "stderr.log".to_string(),
            started_at: 0,
            updated_at: 0,
            timeout_seconds: 0,
            completion_notified: false,
        };
        assert!(is_gateway_owned_task(&task));

        // 兼容旧记录：无 owner 元数据但 label 带 gateway: 前缀
        task.runtime_owner_kind = None;
        task.runtime_owner_id = None;
        task.runtime_process_kind = None;
        assert!(is_gateway_owned_task(&task));

        // 普通后台任务不应被识别为网关
        task.label = "dev server".to_string();
        assert!(!is_gateway_owned_task(&task));
    }

    #[tokio::test]
    async fn refresh_keeps_unlimited_running_task() {
        let mut tasks = vec![BackgroundCommandTask {
            id: "task-1".to_string(),
            runtime_process_id: None,
            runtime_owner_kind: None,
            runtime_owner_id: None,
            runtime_process_kind: None,
            goal_id: None,
            label: "server".to_string(),
            command: "sleep 9999".to_string(),
            cwd: ".".to_string(),
            pid: std::process::id(),
            pgid: None,
            status: "running".to_string(),
            stdout_log: "stdout.log".to_string(),
            stderr_log: "stderr.log".to_string(),
            started_at: 0,
            updated_at: 0,
            timeout_seconds: 0,
            completion_notified: false,
        }];

        refresh_task_statuses(&mut tasks, &AppConfig::default()).await;

        assert_eq!(tasks[0].status, "running");
    }

    #[tokio::test]
    async fn output_read_records_runtime_event_and_output_cap_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let store = BackgroundCommandStore::new(paths.state_dir.clone());
        store.init().unwrap();
        let stdout_log = store.logs_dir().join("task-1.out.log");
        let stderr_log = store.logs_dir().join("task-1.err.log");
        std::fs::write(&stdout_log, "alpha\nbeta\ngamma\n").unwrap();
        std::fs::write(&stderr_log, "").unwrap();
        store
            .save(&[BackgroundCommandTask {
                id: "task-1".to_string(),
                runtime_process_id: Some("background_command_task-1".to_string()),
                runtime_owner_kind: None,
                runtime_owner_id: None,
                runtime_process_kind: None,
                goal_id: None,
                label: "server".to_string(),
                command: "printf lines".to_string(),
                cwd: ".".to_string(),
                pid: 123,
                pgid: Some(123),
                status: "exited".to_string(),
                stdout_log: stdout_log.display().to_string(),
                stderr_log: stderr_log.display().to_string(),
                started_at: 0,
                updated_at: 0,
                timeout_seconds: 0,
                completion_notified: false,
            }])
            .unwrap();
        let mut config = AppConfig::default();
        config.tools.background_command_log_max_bytes = 8;

        let response = read_background_task_output(
            json!({
                "task_id": "task-1",
                "stream": "stdout",
                "tail_lines": 10
            }),
            &config,
            &paths,
        )
        .await
        .unwrap();
        let body: Value = serde_json::from_str(&response).unwrap();
        let snapshot = StateStore::new(&paths)
            .unwrap()
            .session_snapshot(1_000)
            .unwrap();
        let failure = snapshot.runtime_recovery.latest_failure.unwrap();
        let db_path = crate::state::active_state_dir(&paths)
            .unwrap()
            .join("conversation.db");
        let conn = rusqlite::Connection::open(db_path).unwrap();
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM runtime_process_events
                 WHERE process_id = ?1
                 AND stream = 'stdout'
                 AND event_kind = 'output_read'",
                ["background_command_task-1"],
                |row| row.get(0),
            )
            .unwrap();

        assert!(body["stdout"].as_str().unwrap().contains("gamma"));
        assert_eq!(
            failure.kind,
            crate::runtime_recovery::RuntimeRecoveryKind::OutputCapReached
        );
        assert_eq!(
            failure.process_id.as_deref(),
            Some("background_command_task-1")
        );
        assert_eq!(failure.last_safe_seq, Some(1));
        assert_eq!(event_count, 1);
    }
}

use super::store::BackgroundCommandTask;
use crate::runtime_recovery::{
    NewRuntimeProcessEventInput, NewRuntimeProcessRecord, NewRuntimeRecoveryRecord, OwnerKind,
    ProcessKind, RuntimeProcessStatus, RuntimeRecoveryKind, RuntimeRecoveryStatus,
};
use crate::state::StateStore;
use anyhow::Result;

/// 同步后台命令集合到 Runtime Recovery。
///
/// 参数:
/// - `state`: 当前会话状态存储
/// - `tasks`: 后台任务列表
///
/// 返回:
/// - 同步是否成功
pub(super) fn sync_runtime_tasks(
    state: &StateStore,
    tasks: &[BackgroundCommandTask],
) -> Result<()> {
    for task in tasks {
        sync_runtime_task(state, task)?;
    }
    Ok(())
}

/// 同步单个后台命令到 Runtime Recovery。
///
/// 参数:
/// - `state`: 当前会话状态存储
/// - `task`: 后台任务
///
/// 返回:
/// - 同步是否成功
pub(super) fn sync_runtime_task(state: &StateStore, task: &BackgroundCommandTask) -> Result<()> {
    state.record_runtime_process(NewRuntimeProcessRecord {
        id: runtime_process_id_for_task(task),
        session_id: state.session_id().to_string(),
        owner_kind: runtime_owner_kind_from_task(task),
        owner_id: runtime_owner_id_from_task(state, task),
        process_kind: runtime_process_kind_from_task(task),
        command: task.command.clone(),
        cwd: task.cwd.clone(),
        pid: Some(i64::from(task.pid)),
        pgid: task.pgid.map(i64::from),
        status: runtime_status_from_background_task(task.status.as_str()),
        last_seq: 0,
    })
}

/// 记录后台命令输出读取事件。
///
/// 参数:
/// - `state`: 当前会话状态存储
/// - `task`: 后台任务
/// - `stream`: 输出流名称
/// - `log_path`: 输出日志路径
/// - `output`: 读取结果
///
/// 返回:
/// - 记录是否成功
pub(super) fn record_runtime_output_read(
    state: &StateStore,
    task: &BackgroundCommandTask,
    stream: &str,
    log_path: &str,
    output: &LogTail,
) -> Result<()> {
    let process_id = runtime_process_id_for_task(task);
    let seq = state.append_runtime_process_event(NewRuntimeProcessEventInput {
        process_id: process_id.clone(),
        stream: stream.to_string(),
        event_kind: "output_read".to_string(),
        payload_ref: Some(log_path.to_string()),
        payload_preview: output_preview(&output.text),
    })?;
    if output.truncated {
        // 1. 输出被读取上限截断时，保留最后安全序号供压缩与恢复摘要引用
        state.record_runtime_recovery(NewRuntimeRecoveryRecord {
            session_id: state.session_id().to_string(),
            process_id: Some(process_id),
            kind: RuntimeRecoveryKind::OutputCapReached,
            status: RuntimeRecoveryStatus::Observed,
            reason: format!(
                "background command {stream} output exceeded {max_bytes} bytes; read {read_bytes} of {total_bytes} bytes",
                max_bytes = output.max_bytes,
                read_bytes = output.read_bytes,
                total_bytes = output.total_bytes
            ),
            last_safe_seq: Some(seq),
        })?;
    }
    Ok(())
}

/// 生成后台命令对应的运行时进程标识。
///
/// 参数:
/// - `task_id`: 后台任务 ID
///
/// 返回:
/// - 运行时进程标识
pub(super) fn background_runtime_process_id(task_id: &str) -> String {
    format!("background_command_{task_id}")
}

/// 读取后台任务对应的运行时进程标识。
///
/// 参数:
/// - `task`: 后台任务
///
/// 返回:
/// - 运行时进程标识
fn runtime_process_id_for_task(task: &BackgroundCommandTask) -> String {
    task.runtime_process_id
        .clone()
        .unwrap_or_else(|| background_runtime_process_id(&task.id))
}

/// 读取后台任务运行时 owner 类型。
///
/// 参数:
/// - `task`: 后台任务
///
/// 返回:
/// - 运行时 owner 类型
fn runtime_owner_kind_from_task(task: &BackgroundCommandTask) -> OwnerKind {
    task.runtime_owner_kind
        .as_deref()
        .map(OwnerKind::from_str)
        .unwrap_or(OwnerKind::Session)
}

/// 读取后台任务运行时 owner 标识。
///
/// 参数:
/// - `state`: 当前会话状态存储
/// - `task`: 后台任务
///
/// 返回:
/// - 运行时 owner 标识
fn runtime_owner_id_from_task(state: &StateStore, task: &BackgroundCommandTask) -> String {
    task.runtime_owner_id
        .clone()
        .unwrap_or_else(|| state.session_id().to_string())
}

/// 读取后台任务运行时进程类型。
///
/// 参数:
/// - `task`: 后台任务
///
/// 返回:
/// - 运行时进程类型
fn runtime_process_kind_from_task(task: &BackgroundCommandTask) -> ProcessKind {
    task.runtime_process_kind
        .as_deref()
        .map(ProcessKind::from_str)
        .unwrap_or(ProcessKind::BackgroundCommand)
}

/// 将后台任务状态映射为 Runtime Recovery 进程状态。
///
/// 参数:
/// - `status`: 后台任务状态
///
/// 返回:
/// - 运行时进程状态
fn runtime_status_from_background_task(status: &str) -> RuntimeProcessStatus {
    match status {
        "running" => RuntimeProcessStatus::Running,
        "exited" => RuntimeProcessStatus::Exited,
        "stopped" => RuntimeProcessStatus::Stopped,
        "timed_out" => RuntimeProcessStatus::Failed,
        _ => RuntimeProcessStatus::Stale,
    }
}

/// 生成输出事件预览。
///
/// 参数:
/// - `text`: 输出文本
///
/// 返回:
/// - 截断后的输出预览
fn output_preview(text: &str) -> String {
    const MAX_PREVIEW_CHARS: usize = 500;
    text.chars().take(MAX_PREVIEW_CHARS).collect()
}

/// 后台命令日志读取结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LogTail {
    pub(super) text: String,
    pub(super) truncated: bool,
    pub(super) total_bytes: u64,
    pub(super) read_bytes: u64,
    pub(super) max_bytes: u64,
}

impl LogTail {
    /// 创建空日志读取结果。
    ///
    /// 参数:
    /// - `max_bytes`: 最大读取字节数
    ///
    /// 返回:
    /// - 空日志读取结果
    pub(super) fn empty(max_bytes: u64) -> Self {
        Self {
            text: String::new(),
            truncated: false,
            total_bytes: 0,
            read_bytes: 0,
            max_bytes,
        }
    }

    /// 创建日志读取结果。
    ///
    /// 参数:
    /// - `text`: 日志文本
    /// - `truncated`: 是否被读取上限截断
    /// - `total_bytes`: 日志总字节数
    /// - `read_bytes`: 实际读取字节数
    /// - `max_bytes`: 最大读取字节数
    ///
    /// 返回:
    /// - 日志读取结果
    pub(super) fn new(
        text: String,
        truncated: bool,
        total_bytes: u64,
        read_bytes: u64,
        max_bytes: u64,
    ) -> Self {
        Self {
            text,
            truncated,
            total_bytes,
            read_bytes,
            max_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;

    /// 创建测试路径集合。
    ///
    /// 参数:
    /// - `state_dir`: 状态目录
    ///
    /// 返回:
    /// - 测试路径集合
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
    fn maps_background_status_to_runtime_process_status() {
        assert_eq!(
            runtime_status_from_background_task("running"),
            RuntimeProcessStatus::Running
        );
        assert_eq!(
            runtime_status_from_background_task("exited"),
            RuntimeProcessStatus::Exited
        );
        assert_eq!(
            runtime_status_from_background_task("stopped"),
            RuntimeProcessStatus::Stopped
        );
        assert_eq!(
            runtime_status_from_background_task("timed_out"),
            RuntimeProcessStatus::Failed
        );
        assert_eq!(
            runtime_status_from_background_task("unknown"),
            RuntimeProcessStatus::Stale
        );
    }

    #[test]
    fn background_task_sync_records_runtime_owner() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let state = StateStore::new(&paths).unwrap();
        let current_pid = std::process::id();
        let current_pgid = i32::try_from(current_pid).unwrap();
        let task = BackgroundCommandTask {
            id: "task-1".to_string(),
            runtime_process_id: Some("background_command_task-1".to_string()),
            runtime_owner_kind: None,
            runtime_owner_id: None,
            runtime_process_kind: None,
            label: "server".to_string(),
            command: "sleep 9999".to_string(),
            cwd: ".".to_string(),
            pid: current_pid,
            pgid: Some(current_pgid),
            status: "running".to_string(),
            stdout_log: "stdout.log".to_string(),
            stderr_log: "stderr.log".to_string(),
            started_at: 0,
            updated_at: 0,
            timeout_seconds: AppConfig::default()
                .tools
                .background_command_timeout_seconds,
        };

        sync_runtime_task(&state, &task).unwrap();
        let snapshot = state.session_snapshot(1_000).unwrap();

        assert_eq!(snapshot.runtime_recovery.active_process_count, 1);
        assert_eq!(snapshot.runtime_recovery.stale_process_count, 0);
    }

    #[test]
    fn background_task_sync_uses_explicit_gateway_owner() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path().to_path_buf());
        let state = StateStore::new(&paths).unwrap();
        let task = BackgroundCommandTask {
            id: "gateway-qq".to_string(),
            runtime_process_id: Some("background_command_gateway-qq".to_string()),
            runtime_owner_kind: Some("gateway".to_string()),
            runtime_owner_id: Some("qq".to_string()),
            runtime_process_kind: Some("gateway".to_string()),
            label: "gateway:qq".to_string(),
            command: "sai gateway qq-bot".to_string(),
            cwd: ".".to_string(),
            pid: 123,
            pgid: Some(123),
            status: "running".to_string(),
            stdout_log: "stdout.log".to_string(),
            stderr_log: "stderr.log".to_string(),
            started_at: 0,
            updated_at: 0,
            timeout_seconds: AppConfig::default()
                .tools
                .background_command_timeout_seconds,
        };

        sync_runtime_task(&state, &task).unwrap();
        let db_path = crate::state::active_state_dir(&paths)
            .unwrap()
            .join("conversation.db");
        let conn = rusqlite::Connection::open(db_path).unwrap();
        let (owner_kind, owner_id, process_kind): (String, String, String) = conn
            .query_row(
                "SELECT owner_kind, owner_id, process_kind
                 FROM runtime_processes
                 WHERE id = ?1",
                ["background_command_gateway-qq"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert_eq!(owner_kind, "gateway");
        assert_eq!(owner_id, "qq");
        assert_eq!(process_kind, "gateway");
    }
}

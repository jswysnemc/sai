use super::process_store::{GatewayProcessRecord, GatewayProcessStore};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::runtime_recovery::{
    NewRuntimeProcessRecord, OwnerKind, ProcessKind, RuntimeProcessStatus,
};
use crate::state::StateStore;
use crate::tools::command::{
    process_exists, spawn_background_shell, terminate_process, unix_seconds,
};
use anyhow::Result;
use std::time::Duration;

const LEGACY_GATEWAY_OWNER_KIND: &str = "gateway";
const LEGACY_GATEWAY_LABEL_PREFIX: &str = "gateway:";

/// 启动网关独立进程并写入网关进程存储。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway_id`: 网关标识
/// - `command`: shell 命令
/// - `cwd`: 工作目录
///
/// 返回:
/// - 新网关进程记录
pub(crate) fn spawn_gateway_process(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway_id: &str,
    command: &str,
    cwd: &str,
) -> Result<GatewayProcessRecord> {
    let store = GatewayProcessStore::new(paths.state_dir.clone());
    store.init()?;
    let cwd_path = std::path::PathBuf::from(cwd);
    std::fs::create_dir_all(&cwd_path)?;
    let now = unix_seconds();
    let stdout_log = store.logs_dir().join(format!("{now}-{gateway_id}.out.log"));
    let stderr_log = store.logs_dir().join(format!("{now}-{gateway_id}.err.log"));
    let stdout = std::fs::File::create(&stdout_log)?;
    let stderr = std::fs::File::create(&stderr_log)?;
    let process = spawn_background_shell(
        command,
        &cwd_path,
        &config.tools.command_shell,
        stdout,
        stderr,
    )?;
    let record = GatewayProcessRecord {
        gateway_id: gateway_id.to_string(),
        command: command.to_string(),
        cwd: cwd_path.display().to_string(),
        pid: process.pid,
        pgid: process.pgid,
        status: "running".to_string(),
        stdout_log: stdout_log.display().to_string(),
        stderr_log: stderr_log.display().to_string(),
        started_at: now,
        updated_at: now,
    };
    store.replace_gateway_record(record.clone())?;
    sync_gateway_runtime_process(paths, &record)?;
    Ok(record)
}

/// 刷新全部网关进程状态并迁移旧后台任务记录。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 刷新后的网关进程记录列表
pub(crate) fn refresh_gateway_processes(paths: &SaiPaths) -> Result<Vec<GatewayProcessRecord>> {
    migrate_legacy_gateway_tasks(paths)?;
    let store = GatewayProcessStore::new(paths.state_dir.clone());
    let mut records = store.load()?;
    let now = unix_seconds();
    let mut changed = false;
    for record in &mut records {
        // 1. 运行中的记录按真实进程存活状态刷新
        if record.is_running() && !process_exists(record.pid) {
            record.status = "exited".to_string();
            record.updated_at = now;
            changed = true;
        }
    }
    if changed {
        store.save(&records)?;
    }
    for record in &records {
        sync_gateway_runtime_process(paths, record)?;
    }
    Ok(records)
}

/// 停止指定网关进程。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `gateway_id`: 网关标识
///
/// 返回:
/// - 实际停止的进程数量
pub(crate) async fn stop_gateway_process(
    paths: &SaiPaths,
    config: &AppConfig,
    gateway_id: &str,
) -> Result<usize> {
    let store = GatewayProcessStore::new(paths.state_dir.clone());
    let mut records = store.load()?;
    let mut stopped = 0usize;
    for record in &mut records {
        if record.gateway_id != gateway_id || !record.is_running() {
            continue;
        }
        if process_exists(record.pid) {
            // 1. 先温和终止，宽限期后仍存活再强制终止
            terminate_process(record.pid, record.pgid, false).await;
            tokio::time::sleep(Duration::from_secs(
                config.tools.background_command_stop_grace_seconds,
            ))
            .await;
            if process_exists(record.pid) {
                terminate_process(record.pid, record.pgid, true).await;
            }
            stopped += 1;
        }
        record.status = "stopped".to_string();
        record.updated_at = unix_seconds();
    }
    store.save(&records)?;
    for record in records.iter().filter(|item| item.gateway_id == gateway_id) {
        sync_gateway_runtime_process(paths, record)?;
    }
    Ok(stopped)
}

/// 将历史遗留在通用后台任务存储中的网关任务迁移到网关进程存储。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 迁移是否成功
pub(crate) fn migrate_legacy_gateway_tasks(paths: &SaiPaths) -> Result<()> {
    let legacy_store = crate::tools::command::BackgroundCommandStore::new(paths.state_dir.clone());
    let tasks = match legacy_store.load() {
        Ok(tasks) => tasks,
        Err(_) => return Ok(()),
    };
    let (gateway_tasks, other_tasks): (Vec<_>, Vec<_>) =
        tasks.into_iter().partition(is_legacy_gateway_task);
    if gateway_tasks.is_empty() {
        return Ok(());
    }
    let store = GatewayProcessStore::new(paths.state_dir.clone());
    store.init()?;
    let mut records = store.load()?;
    for task in gateway_tasks {
        let gateway_id = legacy_gateway_id(&task);
        // 1. 网关存储中已有同网关记录时优先保留现有记录，仅补充缺失网关
        if records.iter().any(|record| record.gateway_id == gateway_id) {
            continue;
        }
        records.push(GatewayProcessRecord {
            gateway_id,
            command: task.command,
            cwd: task.cwd,
            pid: task.pid,
            pgid: task.pgid,
            status: task.status,
            stdout_log: task.stdout_log,
            stderr_log: task.stderr_log,
            started_at: task.started_at,
            updated_at: task.updated_at,
        });
    }
    store.save(&records)?;
    legacy_store.save(&other_tasks)?;
    Ok(())
}

/// 判断旧后台任务是否属于网关。
///
/// 参数:
/// - `task`: 旧后台任务
///
/// 返回:
/// - 是否属于网关
fn is_legacy_gateway_task(task: &crate::tools::command::BackgroundCommandTask) -> bool {
    task.runtime_owner_kind.as_deref() == Some(LEGACY_GATEWAY_OWNER_KIND)
        || task.label.starts_with(LEGACY_GATEWAY_LABEL_PREFIX)
}

/// 解析旧后台任务对应的网关标识。
///
/// 参数:
/// - `task`: 旧后台任务
///
/// 返回:
/// - 网关标识
fn legacy_gateway_id(task: &crate::tools::command::BackgroundCommandTask) -> String {
    task.runtime_owner_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            task.label
                .strip_prefix(LEGACY_GATEWAY_LABEL_PREFIX)
                .unwrap_or(&task.label)
                .to_string()
        })
}

/// 同步网关进程记录到 Runtime Recovery。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `record`: 网关进程记录
///
/// 返回:
/// - 同步是否成功
fn sync_gateway_runtime_process(paths: &SaiPaths, record: &GatewayProcessRecord) -> Result<()> {
    let state = StateStore::new(paths)?;
    state.record_runtime_process(NewRuntimeProcessRecord {
        id: record.runtime_process_id(),
        session_id: state.session_id().to_string(),
        owner_kind: OwnerKind::Gateway,
        owner_id: record.gateway_id.clone(),
        process_kind: ProcessKind::Gateway,
        command: record.command.clone(),
        cwd: record.cwd.clone(),
        pid: Some(i64::from(record.pid)),
        pgid: record.pgid.map(i64::from),
        status: runtime_status_from_record(record.status.as_str()),
        last_seq: 0,
    })
}

/// 将网关进程状态映射为运行时进程状态。
///
/// 参数:
/// - `status`: 网关进程状态
///
/// 返回:
/// - 运行时进程状态
fn runtime_status_from_record(status: &str) -> RuntimeProcessStatus {
    match status {
        "running" => RuntimeProcessStatus::Running,
        "exited" => RuntimeProcessStatus::Exited,
        "stopped" => RuntimeProcessStatus::Stopped,
        _ => RuntimeProcessStatus::Stale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::command::BackgroundCommandTask;

    fn legacy_task(
        label: &str,
        owner_kind: Option<&str>,
        owner_id: Option<&str>,
    ) -> BackgroundCommandTask {
        BackgroundCommandTask {
            id: "1-100".to_string(),
            runtime_process_id: None,
            runtime_owner_kind: owner_kind.map(str::to_string),
            runtime_owner_id: owner_id.map(str::to_string),
            runtime_process_kind: None,
            label: label.to_string(),
            command: "sai gateway qq-bot".to_string(),
            cwd: ".".to_string(),
            pid: 100,
            pgid: None,
            status: "running".to_string(),
            stdout_log: "stdout.log".to_string(),
            stderr_log: "stderr.log".to_string(),
            started_at: 1,
            updated_at: 1,
            timeout_seconds: 0,
        }
    }

    #[test]
    fn detects_legacy_gateway_tasks_by_owner_and_label() {
        assert!(is_legacy_gateway_task(&legacy_task(
            "gateway:qq",
            None,
            None
        )));
        assert!(is_legacy_gateway_task(&legacy_task(
            "custom",
            Some("gateway"),
            Some("weixin")
        )));
        assert!(!is_legacy_gateway_task(&legacy_task(
            "dev server",
            Some("session"),
            Some("default")
        )));
    }

    #[test]
    fn resolves_gateway_id_from_owner_then_label() {
        assert_eq!(
            legacy_gateway_id(&legacy_task("gateway:qq", Some("gateway"), Some("weixin"))),
            "weixin"
        );
        assert_eq!(
            legacy_gateway_id(&legacy_task("gateway:qq", None, None)),
            "qq"
        );
    }
}

use super::background_refresh::refresh_task_statuses;
use super::background_tasks::{read_log_tail, BackgroundRuntimeOwner};
use super::store::BackgroundCommandStore;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const WAIT_DEFAULT_SECONDS: u64 = 60;
const WAIT_MAX_SECONDS: u64 = 60;
const WAIT_POLL_MILLIS: u64 = 500;

/// 阻塞等待后台任务进入终态。
///
/// 指定 `task_id` 时等待该任务；省略时等待任意运行中的任务。超时只返回状态，
/// 不会停止或修改仍在运行的任务。
///
/// 参数:
/// - `args`: 包含可选 `task_id` 和 `timeout_seconds` 的工具参数
/// - `config`: 应用配置，用于刷新任务状态
/// - `paths`: Sai 路径
/// - `owner`: 调用方会话范围，空值保留命令模式的全局行为
///
/// 返回:
/// - JSON 格式的终态任务、无任务说明或等待超时结果
pub(super) async fn wait_background_task(
    args: Value,
    config: &AppConfig,
    paths: &SaiPaths,
    owner: Option<&BackgroundRuntimeOwner>,
) -> Result<String> {
    let task_id = args
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let timeout_seconds = args
        .get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(WAIT_DEFAULT_SECONDS)
        .clamp(1, WAIT_MAX_SECONDS);
    let started = tokio::time::Instant::now();
    let store = BackgroundCommandStore::new(paths.state_dir.clone());
    let wait_for_any = task_id.is_none();
    let mut tracked_task_ids = None;

    loop {
        let mut tasks = store.load()?;
        // 1. 等待范围限制在调用方所属会话，不能被其他会话任务占住
        if let Some(owner) = owner {
            tasks.retain(|task| {
                task.runtime_owner_id.as_deref() == Some(&owner.owner_id)
                    && task.runtime_owner_kind.as_deref() == Some(owner.owner_kind.as_str())
            });
        }
        if wait_for_any && tracked_task_ids.is_none() {
            let running_ids = tasks
                .iter()
                .filter(|task| task.status == "running")
                .map(|task| task.id.clone())
                .collect::<Vec<_>>();
            if running_ids.is_empty() {
                return Ok(serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "completed": false,
                    "message": "no running background tasks to wait for",
                }))?);
            }
            tracked_task_ids = Some(running_ids);
        }
        refresh_task_statuses(&mut tasks, config).await;
        // 2. 【后台命令】【等待合并】合并进程状态后，使用最新任务表判断等待结果
        tasks = store.merge_statuses(&tasks)?;
        if let Some(owner) = owner {
            tasks.retain(|task| {
                task.runtime_owner_id.as_deref() == Some(&owner.owner_id)
                    && task.runtime_owner_kind.as_deref() == Some(owner.owner_kind.as_str())
            });
        }

        let terminal_task = if let Some(id) = task_id.as_deref() {
            Some(
                tasks
                    .iter()
                    .find(|task| task.id == id)
                    .with_context(|| format!("background command not found: {id}"))?,
            )
        } else {
            tracked_task_ids.as_ref().and_then(|ids| {
                tasks
                    .iter()
                    .find(|task| ids.iter().any(|id| id == &task.id) && task.status != "running")
            })
        };
        if let Some(task) = terminal_task.filter(|task| task.status != "running") {
            return Ok(serde_json::to_string_pretty(&json!({
                "ok": task.status == "exited",
                "completed": true,
                "waited": true,
                "task": task,
            }))?);
        }

        if started.elapsed().as_secs() >= timeout_seconds {
            let task = tasks.iter().find(|task| {
                task_id.as_ref().map_or_else(
                    || {
                        tracked_task_ids
                            .as_ref()
                            .is_some_and(|ids| ids.contains(&task.id))
                    },
                    |id| &task.id == id,
                ) && task.status == "running"
            });
            let max_bytes = config
                .tools
                .background_command_log_max_bytes
                .clamp(1, 8_192);
            let stdout = task
                .map(|task| read_log_tail(&task.stdout_log, 16, max_bytes))
                .transpose()?;
            let stderr = task
                .map(|task| read_log_tail(&task.stderr_log, 16, max_bytes))
                .transpose()?;
            return Ok(serde_json::to_string_pretty(&json!({
                "ok": true,
                "completed": false,
                "timeout": true,
                "waited": true,
                "needs_attention": true,
                "message": "The wait interval ended; the command is still running. Review the recent logs for progress, errors or input prompts before deciding whether to wait again. This does not stop the command.",
                "task_id": task.map(|task| task.id.as_str()),
                "task": task,
                "stdout": stdout.as_ref().map(|output| &output.text),
                "stderr": stderr.as_ref().map(|output| &output.text),
                "stdout_truncated": stdout.as_ref().is_some_and(|output| output.truncated),
                "stderr_truncated": stderr.as_ref().is_some_and(|output| output.truncated),
                "waited_seconds": started.elapsed().as_secs(),
            }))?);
        }
        tokio::time::sleep(Duration::from_millis(WAIT_POLL_MILLIS)).await;
    }
}

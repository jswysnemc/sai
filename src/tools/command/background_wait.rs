use super::background_tasks::refresh_task_statuses;
use super::store::BackgroundCommandStore;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const WAIT_DEFAULT_SECONDS: u64 = 180;
const WAIT_MAX_SECONDS: u64 = 600;
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
///
/// 返回:
/// - JSON 格式的终态任务、无任务说明或等待超时结果
pub(super) async fn wait_background_task(
    args: Value,
    config: &AppConfig,
    paths: &SaiPaths,
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
        if refresh_task_statuses(&mut tasks, config).await {
            store.save(&tasks)?;
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
            return Ok(serde_json::to_string_pretty(&json!({
                "ok": false,
                "timeout": true,
                "waited": true,
                "message": "background task is still running; a later wait or output call can inspect it",
                "task_id": task_id,
            }))?);
        }
        tokio::time::sleep(Duration::from_millis(WAIT_POLL_MILLIS)).await;
    }
}

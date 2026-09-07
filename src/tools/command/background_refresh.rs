use super::background_timeout::is_unlimited;
use super::process::{process_exists, terminate_process};
use super::store::{unix_seconds, BackgroundCommandStore, BackgroundCommandTask};
use crate::config::AppConfig;
use anyhow::Result;
use std::time::Duration;

/// 【后台命令】【状态刷新】检查指定范围内的进程，并合并到最新任务表。
///
/// 参数:
/// - `store`: 后台任务存储
/// - `config`: 超时和停止宽限配置
/// - `matches`: 需要检查的任务范围
///
/// 返回:
/// - 合并后的完整任务表，不恢复已删除记录，也不覆盖消费标记
pub(super) async fn refresh_background_tasks(
    store: &BackgroundCommandStore,
    config: &AppConfig,
    matches: impl Fn(&BackgroundCommandTask) -> bool,
) -> Result<Vec<BackgroundCommandTask>> {
    // 1. 【后台命令】【状态刷新】异步进程检查在事务外执行
    let mut tasks = store.load()?;
    tasks.retain(matches);
    refresh_task_statuses(&mut tasks, config).await;
    // 2. 【后台命令】【状态刷新】重新读取最新状态，只提交进程状态变化
    store.merge_statuses(&tasks)
}

/// 【后台命令】【状态刷新】检查任务快照中的进程，不直接写入存储。
///
/// 参数:
/// - `tasks`: 待更新的任务快照
/// - `config`: 超时和停止宽限配置
///
/// 返回:
/// - 观察到运行状态变化时返回 true
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

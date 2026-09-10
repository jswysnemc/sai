mod process;
mod record;
mod store;
mod worker;
mod worker_runtime;

#[cfg(test)]
mod tests;

pub(crate) use worker::run as run_worker;

use self::record::{Flag, LegacyRecord};
use self::store::Store;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use sai_plugin_runtime::host::{validate_scheduled_id, ScheduledStatus, ScheduledTask};

const ACTIVE: &[ScheduledStatus] = &[
    ScheduledStatus::Scheduled,
    ScheduledStatus::Running,
    ScheduledStatus::Cancelling,
];

/// 【旧闹钟兼容】【列表投影】只读取旧任务与独立状态，不启动或重新投递任务。
/// @param paths 可信应用目录
/// @returns 可以通过通用调度接口展示的旧任务
pub(in crate::plugins) fn list(paths: &SaiPaths) -> Result<Vec<ScheduledTask>> {
    Store::open(paths)?
        .snapshot()?
        .into_iter()
        .map(|(record, flag)| project(paths, &record, flag.as_ref()))
        .collect()
}

/// 【旧闹钟兼容】【单条投影】只接受经过映射的任务标识，旧公开标识由 Lua 处理。
/// @param paths 可信目录；id 为通用任务标识
/// @returns 匹配任务或 None
pub(in crate::plugins) fn get(paths: &SaiPaths, id: &str) -> Result<Option<ScheduledTask>> {
    validate_scheduled_id(id)?;
    Store::open(paths)?
        .snapshot()?
        .into_iter()
        .find(|(record, _)| record.task_id() == id)
        .map(|(record, flag)| project(paths, &record, flag.as_ref()))
        .transpose()
}

/// 【旧闹钟兼容】【观察状态】退出或身份变化的旧进程不能重新执行，查询只返回诊断。
/// @param paths 可信目录；record 为旧记录；flag 为匹配身份的独立状态
/// @returns 通用状态，查询不会持久化观察结果
fn project(paths: &SaiPaths, record: &LegacyRecord, flag: Option<&Flag>) -> Result<ScheduledTask> {
    let status = flag.map(|flag| flag.status).unwrap_or(record.status());
    if status.is_active() && !process::is_worker(record, &paths.state_dir)? {
        let status = if status == ScheduledStatus::Cancelling {
            ScheduledStatus::Cancelled
        } else {
            ScheduledStatus::Failed
        };
        let observed = Flag::new(
            record,
            status,
            Some("legacy worker exited or its identity changed; task was not retried"),
        )?;
        return record.task(observed.status, observed.finished_at, observed.error);
    }
    record.task(
        status,
        flag.and_then(|flag| flag.finished_at),
        flag.and_then(|flag| flag.error.clone()),
    )
}

/// 【旧闹钟兼容】【取消事务】核验稳定句柄后发布取消状态，退出确认前不报告取消成功。
/// @param paths 可信目录；id 为通用任务标识
/// @returns 是否完成一次新的取消；身份不符或平台拒绝时返回错误
pub(in crate::plugins) fn cancel(paths: &SaiPaths, id: &str) -> Result<bool> {
    validate_scheduled_id(id)?;
    let store = Store::open(paths)?;
    let _cancellation = store.cancellation_lock()?;
    let Some((record, flag)) = store
        .snapshot()?
        .into_iter()
        .find(|(record, _)| record.task_id() == id)
    else {
        return Ok(false);
    };
    let previous = flag
        .as_ref()
        .map(|flag| flag.status)
        .unwrap_or(record.status());
    if !previous.is_active() {
        return Ok(false);
    }
    // 1. 【旧闹钟兼容】【取消身份】进程句柄先于状态变更捕获，拒绝按持久 PID 直接发送信号
    let handle = process::capture(&record, &paths.state_dir)?;
    let cancelling = Flag::new(&record, ScheduledStatus::Cancelling, None)?;
    let current = store.transition(&record, Some(cancelling), ACTIVE)?;
    if current.is_none_or(|flag| flag.status != ScheduledStatus::Cancelling) {
        return Ok(false);
    }
    // 2. 【旧闹钟兼容】【退出确认】新入口先响应状态取消，旧入口随后通过稳定句柄结束
    if let Some(handle) = handle {
        if let Err(error) = handle.terminate() {
            let restored = if previous == ScheduledStatus::Cancelling {
                record.status()
            } else {
                previous
            };
            let diagnostic = format!("legacy alarm cancellation failed: {error:#}");
            store
                .transition(
                    &record,
                    Some(Flag::new(&record, restored, Some(&diagnostic))?),
                    &[ScheduledStatus::Cancelling],
                )
                .context("restore legacy alarm after cancellation failure")?;
            return Err(error);
        }
    }
    // 3. 【旧闹钟兼容】【终态提交】工作入口可能已经确认取消，既有终态保持不变
    store.transition(
        &record,
        Some(Flag::new(&record, ScheduledStatus::Cancelled, None)?),
        &[ScheduledStatus::Cancelling],
    )?;
    Ok(true)
}

use super::{
    record::{Flag, LegacyRecord},
    store::{is_busy, Store},
    worker_runtime,
};
use crate::paths::SaiPaths;
use anyhow::{bail, ensure, Context, Result};
use sai_plugin_runtime::{host::ScheduledStatus, InvocationContext};
use std::{path::Path, time::Duration};

/// 【旧闹钟兼容】【旧工作入口】等待原父进程发布完整记录，按既定绝对时间调用 Lua 提醒。
/// @param paths 可信应用路径；id、time、label、audio_file 为保留的旧命令参数
/// @returns 工作结果，不创建或重放旧任务
pub(crate) async fn run(
    paths: &SaiPaths,
    id: &str,
    time: &str,
    label: &str,
    audio_file: Option<&Path>,
) -> Result<()> {
    ensure!(
        id.len() <= 128 && time.len() <= 256 && label.len() <= 4096,
        "legacy alarm worker arguments exceed limits"
    );
    let store = Store::open(paths)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    // 1. 【旧闹钟兼容】【发布握手】参数和当前 PID 必须同时匹配，拒绝独立伪造的工作入口
    let record = loop {
        if let Some(record) = store.records()?.into_iter().find(|record| record.id == id) {
            ensure!(
                record.time == time
                    && record.label == label
                    && record.audio_file.as_deref() == audio_file,
                "legacy alarm worker arguments do not match the published record"
            );
            if let Some(pid) = record.pid {
                ensure!(
                    pid == std::process::id(),
                    "legacy alarm worker PID does not match the published record"
                );
                break record;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("legacy alarm parent did not publish this worker");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let _lease = loop {
        match store.worker_lock(&record) {
            Err(error) if is_busy(&error) && tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(10)).await
            }
            result => break result?,
        }
    };
    // 2. 【旧闹钟兼容】【到期等待】仅待执行状态可进入提醒，运行中断和终态均不能再次执行
    loop {
        match status(&store, &record)? {
            ScheduledStatus::Scheduled => {}
            ScheduledStatus::Cancelling => return finish(&store, &record, None).await,
            _ => return Ok(()),
        }
        if chrono::Utc::now().timestamp() >= record.due_at {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let result = execute(paths, &store, &record).await;
    // 3. 【旧闹钟兼容】【结果保存】取消优先于晚到投递结果，失败不降级为成功或自动重试
    finish(&store, &record, result).await
}

/// 【旧闹钟兼容】【状态核验】每次观察都核对记录身份，不沿用已变化记录的执行资格。
/// @param store 兼容存储；record 为握手时的完整记录
/// @returns 当前状态
fn status(store: &Store, record: &LegacyRecord) -> Result<ScheduledStatus> {
    let current = store
        .records()?
        .into_iter()
        .find(|item| item.id == record.id)
        .context("legacy alarm disappeared")?;
    ensure!(
        current.revision()? == record.revision()?,
        "legacy alarm identity changed"
    );
    Ok(store
        .flag(record)?
        .map(|flag| flag.status)
        .unwrap_or(current.status()))
}

/// 【旧闹钟兼容】【命令执行】业务投递由 Lua 完成，取消状态释放命令及通知 Future。
/// @param paths 可信路径；store 为存储；record 为已核验的旧任务
/// @returns 命令结果，None 表示取消
async fn execute(paths: &SaiPaths, store: &Store, record: &LegacyRecord) -> Option<Result<String>> {
    let runtime = match worker_runtime::load(paths, record) {
        Ok(runtime) => runtime,
        Err(error) => return Some(Err(error)),
    };
    let started = change(
        store,
        record,
        ScheduledStatus::Running,
        None,
        &[ScheduledStatus::Scheduled],
    )
    .await;
    match started {
        Ok(Some(flag)) if flag.status == ScheduledStatus::Running => {}
        Ok(_) => return None,
        Err(error) => return Some(Err(error)),
    }
    let arguments = match record.arguments() {
        Ok(arguments) => arguments,
        Err(error) => return Some(Err(error)),
    };
    let workdir = match crate::runtime_cwd::current_dir() {
        Ok(path) => path,
        Err(error) => return Some(Err(error.into())),
    };
    let context = InvocationContext {
        session_id: format!("legacy-alarm/{}", record.task_id()),
        storage_session_id: format!("legacy-alarm/{}", record.task_id()),
        operation_id: record.task_id(),
        workdir: workdir.display().to_string(),
        allow_writes: true,
        ..Default::default()
    };
    let mut call = Box::pin(runtime.call_command("deliver", &arguments, context));
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            result = &mut call => return Some(result),
            _ = interval.tick() => match status(store, record) {
                Ok(ScheduledStatus::Running) => {},
                Ok(_) => return None,
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

/// 【旧闹钟兼容】【终态处理】无论提醒是否启动，都保存可诊断结果并尊重已提交取消。
/// @param store 存储；record 为旧任务；result 为投递结果
/// @returns 独立终态提交结果
async fn finish(
    store: &Store,
    record: &LegacyRecord,
    result: Option<Result<String>>,
) -> Result<()> {
    let (next, error) = match result {
        Some(Ok(_)) => (ScheduledStatus::Completed, None),
        Some(Err(error)) => (ScheduledStatus::Failed, Some(format!("{error:#}"))),
        None => (ScheduledStatus::Cancelled, None),
    };
    let current = change(
        store,
        record,
        next,
        error.as_deref(),
        &[ScheduledStatus::Scheduled, ScheduledStatus::Running],
    )
    .await?;
    if current.is_some_and(|flag| flag.status == ScheduledStatus::Cancelling) {
        change(
            store,
            record,
            ScheduledStatus::Cancelled,
            None,
            &[ScheduledStatus::Cancelling],
        )
        .await?;
    }
    Ok(())
}

/// 【旧闹钟兼容】【短锁重试】只重试明确的锁占用，数据与身份错误立即返回。
/// @param store 存储；record 为旧任务；next 为状态；error 为诊断；allowed 为允许的前置状态
/// @returns 实际提交后状态
async fn change(
    store: &Store,
    record: &LegacyRecord,
    next: ScheduledStatus,
    error: Option<&str>,
    allowed: &[ScheduledStatus],
) -> Result<Option<Flag>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        match store.transition(record, Some(Flag::new(record, next, error)?), allowed) {
            Err(error) if is_busy(&error) && tokio::time::Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(10)).await
            }
            result => return result,
        }
    }
}

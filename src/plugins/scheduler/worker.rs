use super::{load_current, process, record::JobRecord, store::Store, validate_plugin};
use anyhow::{ensure, Context, Result};
use sai_plugin_runtime::{host::ScheduledStatus, InvocationContext};
use std::path::Path;
use std::time::Duration;

/// 【插件调度】【工作入口】仅按持久任务和启动身份执行，不接受命令或授权覆盖参数。
/// @param state_dir 任务状态根；plugin 为插件；id 为任务；launch 为本次启动身份
/// @returns 工作进程结果，错误保留到持久任务记录
pub(crate) async fn run(state_dir: &Path, plugin: &str, id: &str, launch: &str) -> Result<()> {
    validate_plugin(plugin)?;
    let store = Store::open(state_dir, plugin)?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    // 1. 【插件调度】【启动握手】等待父进程发布 PID，同一任务只允许一个执行锁持有者
    let _lease = loop {
        let Some(record) = store.get(id)? else {
            return Ok(());
        };
        if record.launch != launch || record.task.status != ScheduledStatus::Scheduled {
            return Ok(());
        }
        if let Some(pid) = record.task.pid {
            if pid != std::process::id() {
                return Ok(());
            }
            if let Some(lease) = process::claim(&store.directory, id)? {
                break lease;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    // 2. 【插件调度】【到期等待】按绝对时间检查任务，父进程退出不影响已经发布的调度
    let record = loop {
        let Some(record) = store.get(id)? else {
            return Ok(());
        };
        if record.launch != launch || record.task.status != ScheduledStatus::Scheduled {
            return Ok(());
        }
        if chrono::Utc::now().timestamp() >= record.task.due_at {
            break record;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    let result = execute(&store, &record).await;
    // 3. 【插件调度】【终态提交】失败和取消均不自动重试，取消状态优先于晚到结果
    update(&store, id, launch, |current| {
        current.finish(result);
        Ok(())
    })
    .await
}

/// 【插件调度】【命令执行】到期后复核源码与授权，取消请求释放整个命令 Future。
/// @param store 任务存储；record 为启动时的可信记录
/// @returns 命令结果，None 表示执行取消
async fn execute(store: &Store, record: &JobRecord) -> Option<Result<String>> {
    let loaded = load_current(
        &record.paths,
        &record.plugin,
        &record.revision,
        record.language,
    );
    let (_, runtime) = match loaded {
        Ok(value) => value,
        Err(error) => return Some(Err(error)),
    };
    if let Err(error) = update(store, &record.task.id, &record.launch, |current| {
        ensure!(
            current.task.status == ScheduledStatus::Scheduled,
            "scheduled task was cancelled before execution"
        );
        current.task.status = ScheduledStatus::Running;
        Ok(())
    })
    .await
    {
        return Some(Err(error));
    }
    let context = InvocationContext {
        session_id: format!("scheduled/{}", record.task.id),
        storage_session_id: format!("plugin-job/{}/{}", record.plugin, record.task.id),
        operation_id: record.task.id.clone(),
        workdir: record.workdir.clone(),
        allow_writes: true,
        ..Default::default()
    };
    let mut call =
        Box::pin(runtime.call_command(&record.task.command, &record.task.arguments, context));
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    loop {
        tokio::select! {
            result = &mut call => return Some(result),
            _ = interval.tick() => {
                match store.get(&record.task.id) {
                    Ok(Some(current)) if current.launch == record.launch && current.task.status == ScheduledStatus::Running => {}
                    Ok(_) => return None,
                    Err(error) => return Some(Err(error)),
                }
            }
        }
    }
}

/// 【插件调度】【状态事务】只重试短锁占用，持久错误仍向调用者返回。
/// @param store 存储；id 为任务；launch 为启动身份；change 为一次状态变更
/// @returns 原子提交结果
async fn update<F>(store: &Store, id: &str, launch: &str, change: F) -> Result<()>
where
    F: FnOnce(&mut JobRecord) -> Result<()>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let _lock = loop {
        match store.lock() {
            Ok(lock) => break lock,
            Err(error)
                if error.chain().any(|cause| {
                    matches!(
                        cause.downcast_ref::<std::fs::TryLockError>(),
                        Some(std::fs::TryLockError::WouldBlock)
                    )
                }) =>
            {
                if tokio::time::Instant::now() >= deadline {
                    return Err(error);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(error) => return Err(error),
        }
    };
    let mut current = store.get(id)?.context("scheduled task disappeared")?;
    ensure!(
        current.launch == launch,
        "scheduled launch identity changed"
    );
    change(&mut current)?;
    store.write(&current)
}

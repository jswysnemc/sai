mod process;
mod record;
mod store;
mod worker;

#[cfg(test)]
mod tests;

pub(crate) use worker::run as run_worker;

use self::process::{SystemLauncher, WorkerLauncher};
use self::record::JobRecord;
use self::store::Store;
use super::discovery::{discover, PluginDescriptor};
use super::private::PrivatePluginHost;
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{bail, ensure, Context, Result};
use sai_plugin_runtime::host::{
    ScheduleRequest, ScheduledStatus, ScheduledTask, SchedulerRequest, SchedulerResponse,
    SystemContext,
};
use sai_plugin_runtime::{Capabilities, PluginRuntime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct CancelOperation(Arc<AtomicBool>);

impl Drop for CancelOperation {
    /// 【插件调度】【调用取消】Future 释放后禁止尚未进入发布事务的任务继续启动。
    /// @returns 无
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【插件调度】【宿主入口】独立检查授权和可信写入权限，插件归属不由 Lua 提供。
/// @param paths 应用路径；plugin 为绑定插件；revision 为实例摘要；request 为操作；context 为可信上下文；capabilities 为授权
/// @returns 操作结果
pub(super) async fn execute(
    paths: &SaiPaths,
    plugin: &str,
    revision: Option<&str>,
    request: SchedulerRequest,
    context: &SystemContext,
    capabilities: &Capabilities,
) -> Result<SchedulerResponse> {
    request.authorize(capabilities, context.allow_writes)?;
    let (paths, plugin, revision, context) = (
        paths.clone(),
        plugin.to_string(),
        revision.map(str::to_string),
        context.clone(),
    );
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOperation(cancelled.clone());
    tokio::task::spawn_blocking(move || {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "scheduled operation was cancelled"
        );
        execute_blocking(
            &paths,
            &plugin,
            revision.as_deref(),
            request,
            &context,
            &cancelled,
        )
    })
    .await
    .context("scheduled operation worker stopped")?
}

/// 【插件调度】【阻塞操作】文件与源码检查位于专用线程，提交前复核调用取消标志。
/// @param paths 路径；plugin 为归属；revision 为摘要；request 为请求；context 为可信上下文；cancelled 为取消标志
/// @returns 操作结果
fn execute_blocking(
    paths: &SaiPaths,
    plugin: &str,
    revision: Option<&str>,
    request: SchedulerRequest,
    context: &SystemContext,
    cancelled: &AtomicBool,
) -> Result<SchedulerResponse> {
    match request {
        SchedulerRequest::Schedule(request) => {
            let revision = revision.context("scheduler requires a bound plugin source revision")?;
            let task = schedule(
                paths,
                plugin,
                revision,
                request,
                context,
                &SystemLauncher,
                cancelled,
            )?;
            Ok(SchedulerResponse::Task(Some(task)))
        }
        SchedulerRequest::List(options) => Ok(SchedulerResponse::Tasks(
            list(paths, plugin)?
                .into_iter()
                .skip(options.offset)
                .take(options.limit)
                .collect(),
        )),
        SchedulerRequest::Get(id) => Ok(SchedulerResponse::Task(get(paths, plugin, &id)?)),
        SchedulerRequest::Cancel(id) => Ok(SchedulerResponse::Changed(cancel(paths, plugin, &id)?)),
        SchedulerRequest::Resume(id) => Ok(SchedulerResponse::Task(Some(resume(
            paths,
            plugin,
            &id,
            &SystemLauncher,
            cancelled,
        )?))),
    }
}

/// 【插件调度】【创建任务】先确认当前源码、授权与目标命令，再原子发布持久记录和进程标识。
/// @param paths 路径；plugin 为插件；revision 为实例摘要；request 为请求；context 为目录；launcher 为进程边界；cancelled 为取消标志
/// @returns 已发布任务
fn schedule(
    paths: &SaiPaths,
    plugin: &str,
    revision: &str,
    request: ScheduleRequest,
    context: &SystemContext,
    launcher: &dyn WorkerLauncher,
    cancelled: &AtomicBool,
) -> Result<ScheduledTask> {
    // 1. 【插件调度】【执行契约】过期实例不能把未来执行交给已经改变的源码或授权
    request.validate()?;
    let (_, runtime) = load_current(paths, plugin, revision, None)?;
    ensure!(
        runtime
            .commands()
            .iter()
            .any(|command| command.name == request.command),
        "scheduled command is not registered"
    );
    let workdir = super::system::paths::workdir(context)?;
    ensure!(
        workdir.is_dir(),
        "scheduled working directory is not a directory"
    );
    // 2. 【插件调度】【记录事务】创建和配额检查在同一锁中完成，不驱逐活动任务
    let store = Store::open(&paths.state_dir, plugin)?;
    let _lock = store.lock()?;
    store.reserve()?;
    let mut record = JobRecord::new(
        plugin,
        revision,
        request,
        paths,
        workdir.display().to_string(),
    );
    record.paths.state_dir = dunce::canonicalize(&paths.state_dir)?;
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "scheduled operation was cancelled before publication"
    );
    start(&store, &mut record, launcher)?;
    Ok(record.task)
}

/// 【插件调度】【启动事务】发布握手防止工作进程读取到半完成记录，启动失败保留有界诊断。
/// @param store 已持操作锁的存储；record 为待启动记录；launcher 为进程创建边界
/// @returns 已发布 PID 的记录；失败时回收尚未发布的子进程
fn start(store: &Store, record: &mut JobRecord, launcher: &dyn WorkerLauncher) -> Result<()> {
    let _lease = process::claim(&store.directory, &record.task.id)?
        .context("scheduled worker is already active")?;
    record.launch = uuid::Uuid::new_v4().simple().to_string();
    record.task.pid = None;
    store.write(record)?;
    let child = match launcher.spawn(record) {
        Ok(child) => child,
        Err(error) => {
            record.finish(Some(Err(anyhow::anyhow!(
                "start scheduled worker: {error:#}"
            ))));
            store.write(record)?;
            return Err(error);
        }
    };
    record.task.pid = Some(child.id());
    store.write(record)?;
    child.commit();
    Ok(())
}

/// 【插件调度】【读取列表】管理入口可以读取禁用或已经卸载插件的任务。
/// @param paths 应用路径；plugin 为任务所属插件
/// @returns 有界任务列表，不执行后台恢复
pub(crate) fn list(paths: &SaiPaths, plugin: &str) -> Result<Vec<ScheduledTask>> {
    validate_plugin(plugin)?;
    let tasks: Vec<_> = Store::open(&paths.state_dir, plugin)?
        .list()?
        .into_iter()
        .map(|record| record.task)
        .collect();
    Ok(tasks)
}

/// 【插件调度】【读取任务】根据已校验标识读取当前插件的一条记录。
/// @param paths 应用路径；plugin 为插件；id 为任务标识
/// @returns 任务或 None
pub(crate) fn get(paths: &SaiPaths, plugin: &str, id: &str) -> Result<Option<ScheduledTask>> {
    validate_plugin(plugin)?;
    if let Some(record) = Store::open(&paths.state_dir, plugin)?.get(id)? {
        return Ok(Some(record.task));
    }
    Ok(None)
}

/// 【插件调度】【请求取消】只取消当前插件命名空间中的公共任务。
/// @param paths 应用路径；plugin 为插件；id 为任务标识
/// @returns 是否接受新的取消请求
pub(crate) fn cancel(paths: &SaiPaths, plugin: &str, id: &str) -> Result<bool> {
    validate_plugin(plugin)?;
    let store = Store::open(&paths.state_dir, plugin)?;
    let _lock = store.lock()?;
    let Some(mut record) = store.get(id)? else {
        return Ok(false);
    };
    if !record.task.status.is_active() {
        return Ok(false);
    }
    let lease = process::claim(&store.directory, id)?;
    if record.task.status == ScheduledStatus::Scheduled || lease.is_some() {
        record.finish(None);
    } else {
        record.task.status = ScheduledStatus::Cancelling;
    }
    store.write(&record)?;
    Ok(true)
}

/// 【插件调度】【恢复入口】恢复尚未开始的任务，已经开始的中断执行不会重新投递。
/// @param paths 应用路径；plugin 为插件；id 为任务标识
/// @returns 恢复后的任务状态
pub(crate) fn resume_task(paths: &SaiPaths, plugin: &str, id: &str) -> Result<ScheduledTask> {
    resume(paths, plugin, id, &SystemLauncher, &AtomicBool::new(false))
}

/// 【插件调度】【恢复事务】执行锁证明旧工作进程已经退出，避免把 PID 复用误判为任务存续。
/// @param paths 应用路径；plugin 为插件；id 为任务；launcher 为启动边界；cancelled 为取消标志
/// @returns 新启动任务或已中断执行的明确终态
fn resume(
    paths: &SaiPaths,
    plugin: &str,
    id: &str,
    launcher: &dyn WorkerLauncher,
    cancelled: &AtomicBool,
) -> Result<ScheduledTask> {
    validate_plugin(plugin)?;
    let store = Store::open(&paths.state_dir, plugin)?;
    let _lock = store.lock()?;
    let mut record = match store.get(id)? {
        Some(record) => record,
        None => {
            drop(_lock);
            bail!("scheduled task not found");
        }
    };
    ensure!(
        record.task.status.is_active(),
        "finished scheduled tasks cannot be resumed"
    );
    let lease =
        process::claim(&store.directory, id)?.context("scheduled worker is already active")?;
    if record.task.status != ScheduledStatus::Scheduled {
        record.finish(Some(Err(anyhow::anyhow!(
            "worker stopped during execution; task was not retried"
        ))));
        store.write(&record)?;
        return Ok(record.task);
    }
    load_current(&record.paths, plugin, &record.revision, record.language)?;
    ensure!(
        !cancelled.load(Ordering::Acquire),
        "scheduled operation was cancelled before publication"
    );
    drop(lease);
    start(&store, &mut record, launcher)?;
    Ok(record.task)
}

/// 【插件调度】【到期授权】读取当前磁盘配置与包，要求启用、调度授权和完整摘要保持一致。
/// @param paths 宿主路径；plugin 为插件；revision 为创建时摘要；language 为创建时语言，旧记录可省略
/// @returns 独立命令运行时，不附加模型或工具调用服务
fn load_current(
    paths: &SaiPaths,
    plugin: &str,
    revision: &str,
    language: Option<crate::i18n::Locale>,
) -> Result<(PluginDescriptor, PluginRuntime)> {
    // 1. 【插件调度】【语言快照】重建派生设置时复用创建语言，实际配置和授权仍须完整匹配
    crate::i18n::with_locale(language.unwrap_or_else(crate::i18n::locale), || {
        let config = AppConfig::load_or_default(paths)?;
        let descriptor = discover(&config, paths)
            .plugins
            .into_iter()
            .find(|descriptor| descriptor.package.manifest.id == plugin)
            .context("scheduled plugin is unavailable")?;
        ensure!(descriptor.setting.enabled, "scheduled plugin is disabled");
        ensure!(
            descriptor
                .capabilities()
                .intersection(&descriptor.grants())
                .system
                .schedule,
            "plugin scheduling grant was revoked"
        );
        ensure!(
            descriptor.revision()? == revision,
            "scheduled plugin source, settings or grants changed"
        );
        let host = Arc::new(PrivatePluginHost::for_descriptor(paths, &descriptor)?);
        let runtime = PluginRuntime::load(
            descriptor.runtime_package(),
            descriptor.settings().clone(),
            descriptor.grants(),
            host,
        )?;
        Ok((descriptor, runtime))
    })
}

/// 【插件调度】【管理标识】限制 CLI 插件标识，任意输入不能改变命名空间结构。
/// @param plugin 插件标识
/// @returns 标识合法时成功
fn validate_plugin(plugin: &str) -> Result<()> {
    if plugin.is_empty()
        || plugin.len() > 32
        || !plugin.as_bytes()[0].is_ascii_lowercase()
        || !plugin.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
    {
        bail!("invalid scheduled plugin id");
    }
    Ok(())
}

pub(super) mod cancel;
pub(super) mod lock;
pub(super) mod paths;
mod target;
mod trash;

use anyhow::{Context, Result};
use sai_plugin_runtime::{
    host::{FileRemovalKind, FileRemovalRequest, SystemContext},
    Capabilities,
};
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

/// 【插件文件】【待提交操作】准备结果继续持锁，迟到结果只清理回收站预留信息
struct Prepared {
    target: target::Target,
    trash: Option<trash::Entry>,
    _lock: lock::Lock,
}

/// 【插件文件】【受限删除】阻塞线程只准备句柄与元数据，成功等待后才开始删除或移动
/// @param state_dir 可信状态目录；request 为目标和类型；context 为可信调用；capabilities 为授权
/// @returns 成功删除为 true，授权目标缺失为 false
pub(super) async fn execute(
    state_dir: PathBuf,
    request: FileRemovalRequest,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<bool> {
    capabilities
        .system
        .check_removal_request(&request.path, request.kind, context.allow_writes)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = cancel::CancelOnDrop(cancelled.clone());
    let prepared = tokio::task::spawn_blocking(move || {
        prepare(&state_dir, &request, &context, &capabilities, &cancelled)
    })
    .await
    .context("plugin file removal worker stopped")??;
    let Some(mut prepared) = prepared else {
        return Ok(false);
    };
    // 1. 【插件文件】【取消边界】只有等待成功的调用会进入提交，已经开始的系统调用不能回滚或强行中断
    match prepared.trash.as_mut() {
        Some(entry) => entry.commit(&prepared.target),
        None => prepared.target.remove(),
    }
}

/// 【插件文件】【准备阶段】授权后取得共用锁，普通文件检查及回收站信息准备均不删除源文件
/// @param state_dir 锁目录；request 为目标；context 为可信目录；capabilities 为授权；cancelled 为撤销标记
/// @returns 持锁准备结果或目标缺失状态
fn prepare(
    state_dir: &Path,
    request: &FileRemovalRequest,
    context: &SystemContext,
    capabilities: &Capabilities,
    cancelled: &AtomicBool,
) -> Result<Option<Prepared>> {
    // 1. 【插件文件】【授权后互斥】越界请求不得创建状态锁或其他目录
    cancel::check_cancelled(cancelled)?;
    let canonical = paths::authorize(request, context, capabilities)?;
    let lock = lock::acquire(state_dir, cancelled)?;
    lock.check_target(&canonical)?;
    cancel::check_cancelled(cancelled)?;
    let Some(target) = target::Target::open(canonical)? else {
        return Ok(None);
    };
    lock.check_object(&target.metadata)?;
    // 2. 【插件文件】【回收站准备】只有回收站模式可以预留系统回收站条目，错误不会退回永久删除
    let trash = match request.kind {
        FileRemovalKind::Permanent => None,
        FileRemovalKind::Trash => Some(trash::prepare(&target, cancelled)?),
    };
    let prepared = Prepared {
        target,
        trash,
        _lock: lock,
    };
    cancel::check_cancelled(cancelled)?;
    Ok(Some(prepared))
}

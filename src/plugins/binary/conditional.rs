use super::{files, paths, revision};
use crate::plugins::file_ops::lock as file_lock;
use anyhow::{Context, Result};
use sai_plugin_runtime::{
    host::{BinaryConditionalWrite, BinaryData, BinaryRevision, SystemContext},
    Capabilities,
};
use std::{
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

/// 【插件写入】【条件发布】在正式宿主共用锁下比较修订、完整暂存，成功等待后才发布
/// @param state_dir 可信状态目录；request 为目标及条件；data 为租约；context 为目录；capabilities 为授权
/// @returns 发布成功为 true，修订不匹配为 false；权限和执行错误不伪装成冲突
pub(in crate::plugins) async fn write(
    state_dir: PathBuf,
    request: BinaryConditionalWrite,
    data: BinaryData,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<bool> {
    capabilities
        .binary
        .check_write(&request.path, context.allow_writes)?;
    capabilities.system.check_read_request(&request.path)?;
    super::validate_limit(request.max_bytes)?;
    files::validate_data(&data)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = files::CancelOnDrop(cancelled.clone());
    let staging = tokio::task::spawn_blocking(move || {
        // 1. 【插件写入】【比较到暂存】工作线程始终持有缓冲租约，成功暂存后把锁一起交给待发布结果
        prepare(&state_dir, request, data, context, capabilities, &cancelled)
    })
    .await
    .context("plugin conditional binary writer stopped")??;
    let Some(staging) = staging else {
        return Ok(false);
    };
    // 2. 【插件写入】【取消边界】取消后不会执行发布，迟到结果会清理暂存文件并释放互斥
    files::publish(&staging)?;
    Ok(true)
}

/// 【插件写入】【条件准备】先完成双重路径授权，冲突时不创建输出父目录或暂存文件
/// @param state_dir 锁目录；request 为条件；data 为缓冲；context 为目录；capabilities 为授权；cancelled 为撤销
/// @returns 持锁的完整暂存结果，条件失败返回 None
fn prepare(
    state_dir: &Path,
    request: BinaryConditionalWrite,
    data: BinaryData,
    context: SystemContext,
    capabilities: Capabilities,
    cancelled: &AtomicBool,
) -> Result<Option<files::Staging>> {
    // 1. 【插件写入】【双重授权】创建锁文件前完成读取与写入范围检查
    files::check_cancelled(cancelled)?;
    let plan = paths::plan(&request.path, &context, &capabilities)?;
    plan.check_read(&context, &capabilities)?;
    // 2. 【插件写入】【持锁观察】互斥覆盖目标检查，缺失父目录不会满足摘要条件
    let lock = file_lock::acquire(state_dir, cancelled)?;
    lock.check_target(&plan.canonical)?;
    files::check_cancelled(cancelled)?;
    let destination = match plan.existing()? {
        Some(destination) => destination,
        None if request.expected == BinaryRevision::Missing => plan.create()?,
        None => return Ok(None),
    };
    // 3. 【插件写入】【比较后暂存】冲突直接释放数据与锁，匹配结果把互斥转交给暂存守卫
    if !revision::matches(
        &destination,
        &request.expected,
        request.max_bytes,
        cancelled,
    )? {
        return Ok(None);
    }
    files::prepare(destination, data, Some(lock), cancelled).map(Some)
}

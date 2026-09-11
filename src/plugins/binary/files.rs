use super::paths;
pub(super) use crate::plugins::file_ops::cancel::{check_cancelled, CancelOnDrop};
use crate::plugins::file_ops::lock as file_lock;
use anyhow::{bail, Context, Result};
use cap_std::fs::{Dir, OpenOptions};
use sai_plugin_runtime::{
    host::{BinaryData, BinaryFile, SystemContext},
    Capabilities,
};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};

/// 【插件写入】【暂存守卫】成功等待后才发布，取消或错误时删除未完成的临时文件。
pub(super) struct Staging {
    destination: paths::Destination,
    name: String,
    bytes: usize,
    _lock: Option<file_lock::Lock>,
}

impl Drop for Staging {
    /// 【插件写入】【暂存清理】释放结果时清理仍然存在的临时文件。
    /// @returns 无；发布后临时名称已经不存在
    fn drop(&mut self) {
        let _ = self.destination.directory.remove_file(&self.name);
    }
}

/// 【插件写入】【原子输出】在线程内写入有预算的缓冲，成功等待后原子替换目标。
/// @param path 目标；data 为缓冲租约；context 为可信工作目录；capabilities 为写入授权
/// @returns 实际路径和已发布字节数
pub(in crate::plugins) async fn write(
    path: String,
    data: BinaryData,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<BinaryFile> {
    write_inner(None, path, data, context, capabilities).await
}

/// 【插件写入】【正式宿主互斥】普通输出与条件输出共用应用状态目录中的稳定锁
/// @param state_dir 可信状态目录；path 为目标；data 为缓冲；context 为目录；capabilities 为授权
/// @returns 成功发布后的路径及完整字节数
pub(in crate::plugins) async fn write_locked(
    state_dir: PathBuf,
    path: String,
    data: BinaryData,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<BinaryFile> {
    write_inner(Some(state_dir), path, data, context, capabilities).await
}

/// 【插件写入】【共享执行】把路径检查、可选互斥与有预算的缓冲一起移入实际文件线程
/// @param state_dir 可选锁目录；path 为目标；data 为预算租约；context 为可信目录；capabilities 为授权
/// @returns 完整成功等待并发布后的文件信息
async fn write_inner(
    state_dir: Option<PathBuf>,
    path: String,
    data: BinaryData,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<BinaryFile> {
    capabilities
        .binary
        .check_write(&path, context.allow_writes)?;
    validate_data(&data)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(cancelled.clone());
    let staging = tokio::task::spawn_blocking(move || {
        // 1. 【插件写入】【预算租约】工作线程持有完整缓冲，调用取消不能提前归还在用字节
        check_cancelled(&cancelled)?;
        let plan = paths::plan(&path, &context, &capabilities)?;
        let lock = state_dir
            .map(|root| file_lock::acquire(&root, &cancelled))
            .transpose()?;
        if let Some(lock) = &lock {
            lock.check_target(&plan.canonical)?;
        }
        check_cancelled(&cancelled)?;
        prepare(plan.create()?, data, lock, &cancelled)
    })
    .await
    .context("plugin binary output worker stopped")??;
    publish(&staging)?;
    Ok(BinaryFile {
        path: staging.destination.display.to_string_lossy().into_owned(),
        bytes: staging.bytes,
    })
}

/// 【插件写入】【暂存准备】授权后创建随机临时文件，每个数据块之前检查取消。
/// @param destination 授权目录句柄；data 为输入；lock 为可选互斥；cancelled 为取消信号
/// @returns 写入完成但尚未发布的暂存文件，继续持锁直到发布或清理结束
pub(super) fn prepare(
    destination: paths::Destination,
    data: BinaryData,
    lock: Option<file_lock::Lock>,
    cancelled: &AtomicBool,
) -> Result<Staging> {
    // 1. 【插件写入】【随机暂存】互斥随暂存对象持有，后续任意错误都会先清理临时文件
    check_cancelled(cancelled)?;
    let name = format!(".sai-binary-{}.tmp", uuid::Uuid::new_v4());
    let staging = Staging {
        destination,
        name,
        bytes: data.bytes().len(),
        _lock: lock,
    };
    let mut file = staging
        .destination
        .directory
        .open_with(
            &staging.name,
            OpenOptions::new().write(true).create_new(true),
        )
        .context("create plugin binary staging file")?;
    // 2. 【插件写入】【分块取消】每个块之前检查撤销，实际线程结束前保持输入预算
    for chunk in data.bytes().chunks(64 * 1024) {
        check_cancelled(cancelled)?;
        file.write_all(chunk)
            .context("write plugin binary staging file")?;
    }
    // 3. 【插件写入】【完整交接】同步并关闭暂存文件后才返回，发布仍由成功等待的调用完成
    file.sync_all()
        .context("flush plugin binary staging file")?;
    drop(file);
    check_cancelled(cancelled)?;
    Ok(staging)
}

/// 【插件写入】【发布】同一个目录句柄内重命名，不跟随目标名称上的链接。
/// @param staging 已完整写入的文件
/// @returns 目标原子替换成功时成功
pub(super) fn publish(staging: &Staging) -> Result<()> {
    let directory: &Dir = &staging.destination.directory;
    directory
        .rename(&staging.name, directory, &staging.destination.name)
        .context("publish plugin binary output")
}

/// 【插件写入】【硬字节上限】直接调用宿主也不能写入超过 64 MiB 的单个缓冲
/// @param data 已保留预算的输入缓冲
/// @returns 数据大小受支持时成功，空文件允许输出
pub(super) fn validate_data(data: &BinaryData) -> Result<()> {
    if data.bytes().len() > 64 * 1024 * 1024 {
        bail!("plugin binary output exceeds byte limit");
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/staging.rs"]
mod tests;

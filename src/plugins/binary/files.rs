use super::paths;
use anyhow::{bail, Context, Result};
use cap_std::fs::{Dir, OpenOptions};
use sai_plugin_runtime::{
    host::{BinaryData, BinaryFile, SystemContext},
    Capabilities,
};
use std::io::Write;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// 【插件写入】【暂存守卫】成功等待后才发布，取消或错误时删除未完成的临时文件。
struct Staging {
    destination: paths::Destination,
    name: String,
    bytes: usize,
}

impl Drop for Staging {
    /// 【插件写入】【暂存清理】释放结果时清理仍然存在的临时文件。
    /// @returns 无；发布后临时名称已经不存在
    fn drop(&mut self) {
        let _ = self.destination.directory.remove_file(&self.name);
    }
}

struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    /// 【插件写入】【取消信号】异步调用被释放时通知阻塞线程停止写入。
    /// @returns 无
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
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
    capabilities
        .binary
        .check_write(&path, context.allow_writes)?;
    if data.bytes().len() > 64 * 1024 * 1024 {
        bail!("plugin binary output exceeds byte limit");
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(cancelled.clone());
    let staging = tokio::task::spawn_blocking(move || {
        // 1. 【插件写入】【预算租约】工作线程持有完整缓冲，调用取消不能提前归还在用字节
        prepare(path, data, context, capabilities, &cancelled)
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
/// @param path 目标；data 为输入；context 为目录；capabilities 为授权；cancelled 为取消信号
/// @returns 写入完成但尚未发布的暂存文件
fn prepare(
    path: String,
    data: BinaryData,
    context: SystemContext,
    capabilities: Capabilities,
    cancelled: &AtomicBool,
) -> Result<Staging> {
    if cancelled.load(Ordering::Acquire) {
        bail!("plugin binary write cancelled");
    }
    let destination = paths::authorize(&path, &context, &capabilities)?;
    let name = format!(".sai-binary-{}.tmp", uuid::Uuid::new_v4());
    let staging = Staging {
        destination,
        name,
        bytes: data.bytes().len(),
    };
    let mut file = staging
        .destination
        .directory
        .open_with(
            &staging.name,
            OpenOptions::new().write(true).create_new(true),
        )
        .context("create plugin binary staging file")?;
    for chunk in data.bytes().chunks(64 * 1024) {
        if cancelled.load(Ordering::Acquire) {
            bail!("plugin binary write cancelled");
        }
        file.write_all(chunk)
            .context("write plugin binary staging file")?;
    }
    file.sync_all()
        .context("flush plugin binary staging file")?;
    drop(file);
    if cancelled.load(Ordering::Acquire) {
        bail!("plugin binary write cancelled");
    }
    Ok(staging)
}

/// 【插件写入】【发布】同一个目录句柄内重命名，不跟随目标名称上的链接。
/// @param staging 已完整写入的文件
/// @returns 目标原子替换成功时成功
fn publish(staging: &Staging) -> Result<()> {
    let directory: &Dir = &staging.destination.directory;
    directory
        .rename(&staging.name, directory, &staging.destination.name)
        .context("publish plugin binary output")
}

use crate::plugins::system::paths;
use anyhow::{bail, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use sai_plugin_runtime::{
    host::{BinaryData, BinaryReadBuffer, SystemContext},
    Capabilities,
};
use std::io::{ErrorKind, Read};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct CancelOnDrop(Arc<AtomicBool>);

impl Drop for CancelOnDrop {
    /// 【插件读取】【取消信号】异步调用释放时通知实际工作线程停止
    /// @returns 无；信号不能提前归还线程仍持有的预算
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【插件读取】【完整文件】在读取前确认预算与授权，把预留缓冲交给实际阻塞线程
/// @param path 请求路径；buffer 为读取预算；context 为可信目录；capabilities 为授权
/// @returns 完整原始字节，取消、超限和特殊文件均返回错误
pub(in crate::plugins) async fn read_file(
    path: String,
    buffer: BinaryReadBuffer,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<BinaryData> {
    capabilities.system.check_read_request(&path)?;
    super::validate_limit(buffer.max_bytes())?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(cancelled.clone());
    tokio::task::spawn_blocking(move || {
        // 1. 【插件读取】【预算租约】取消后的预留额度随工作线程一起释放，排队期间也不能重复使用
        read_authorized(&path, buffer, &context, &capabilities, &cancelled)
    })
    .await
    .context("plugin binary file reader stopped")?
}

/// 【插件读取】【安全打开】先按目录句柄授权，再拒绝特殊对象与校验后替换的末级链接
/// @param path 路径；buffer 为预算；context 为可信目录；capabilities 为授权；cancelled 为取消标记
/// @returns 已读取完整文件的不可变缓冲
fn read_authorized(
    path: &str,
    buffer: BinaryReadBuffer,
    context: &SystemContext,
    capabilities: &Capabilities,
    cancelled: &AtomicBool,
) -> Result<BinaryData> {
    check_cancelled(cancelled)?;
    let path = paths::authorize(path, context, capabilities)?;
    check_cancelled(cancelled)?;
    if !path.directory.symlink_metadata(&path.relative)?.is_file() {
        bail!("plugin binary reading requires an unchanged regular file");
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    // 2. 【插件读取】【对象复核】打开后的句柄再次验证类型，非阻塞打开避免替换成管道后一直等待
    let mut file = path
        .directory
        .open_with(&path.relative, &options)
        .context("open plugin binary file")?;
    let metadata = file.metadata().context("inspect plugin binary file")?;
    if !metadata.is_file() {
        bail!("plugin binary reading requires a regular file");
    }
    if metadata.len() > buffer.max_bytes() as u64 {
        bail!("plugin binary file exceeds size limit");
    }
    read_chunks(&mut file, buffer, cancelled)
}

/// 【插件读取】【有界分块】持续读取到 EOF，额外探测一字节以识别读取过程中增长的文件
/// @param reader 已打开的文件；buffer 为预留空间；cancelled 为取消标记
/// @returns 仅在完整读取成功且未取消时转换缓冲，不提供截断结果
fn read_chunks(
    reader: &mut impl Read,
    mut buffer: BinaryReadBuffer,
    cancelled: &AtomicBool,
) -> Result<BinaryData> {
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        check_cancelled(cancelled)?;
        let wanted = buffer.remaining().min(chunk.len()).max(1);
        match reader.read(&mut chunk[..wanted]) {
            Ok(0) => break,
            Ok(count) => buffer.extend_from_slice(&chunk[..count])?,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("read plugin binary file"),
        }
    }
    check_cancelled(cancelled)?;
    Ok(buffer.finish())
}

/// 【插件读取】【取消检查】在授权、数据块和结果转换边界拒绝已经撤销的读取
/// @param cancelled 来自异步调用守卫的标记
/// @returns 读取仍有效时成功
fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        bail!("plugin binary read cancelled");
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/read.rs"]
mod tests;

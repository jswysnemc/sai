use super::{files::check_cancelled, paths::Destination};
use anyhow::{bail, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use sai_plugin_runtime::host::BinaryRevision;
use sha2::{Digest, Sha256};
use std::{
    io::{ErrorKind, Read},
    sync::atomic::AtomicBool,
};

/// 【插件写入】【文件修订比较】在同一锁保护下比较普通文件是否存在或完整摘要是否匹配
/// @param destination 授权目标；expected 为条件；max_bytes 为旧文件上限；cancelled 为取消标记
/// @returns 条件匹配为 true，特殊对象与读取错误明确失败
pub(super) fn matches(
    destination: &Destination,
    expected: &BinaryRevision,
    max_bytes: usize,
    cancelled: &AtomicBool,
) -> Result<bool> {
    check_cancelled(cancelled)?;
    match destination.directory.symlink_metadata(&destination.name) {
        Ok(metadata) if !metadata.is_file() => {
            bail!("plugin conditional binary output requires a regular file")
        }
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(matches!(expected, BinaryRevision::Missing))
        }
        Err(error) => return Err(error).context("inspect conditional binary output"),
    }
    let BinaryRevision::Sha256(expected) = expected else {
        return Ok(false);
    };
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    // 1. 【插件写入】【比较句柄】打开前后确认普通文件，拒绝末级替换链接，非阻塞打开防止管道替换导致等待
    let mut file = match destination.directory.open_with(&destination.name, &options) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("open conditional binary output"),
    };
    let metadata = file
        .metadata()
        .context("inspect opened conditional binary output")?;
    if !metadata.is_file() {
        bail!("plugin conditional binary output requires a regular file");
    }
    if metadata.len() > max_bytes as u64 {
        bail!("plugin binary comparison exceeds size limit");
    }
    Ok(digest(&mut file, max_bytes, cancelled)? == *expected)
}

/// 【插件写入】【有界摘要】流式计算完整文件摘要，读满上限后额外检查一字节以拒绝增长
/// @param reader 普通文件读取器；max_bytes 为有效上限；cancelled 为撤销标记
/// @returns 完整读取到 EOF 且未取消时返回 SHA-256，不对截断内容计算修订
fn digest(reader: &mut impl Read, max_bytes: usize, cancelled: &AtomicBool) -> Result<[u8; 32]> {
    let mut hash = Sha256::new();
    let mut total = 0;
    let mut chunk = [0_u8; 64 * 1024];
    // 1. 【插件写入】【完整比较】即使已经读满上限，也要探测 EOF，避免把增长文件按前缀比较
    loop {
        check_cancelled(cancelled)?;
        let remaining = max_bytes - total;
        let wanted = remaining.min(chunk.len()).max(1);
        match reader.read(&mut chunk[..wanted]) {
            Ok(0) => break,
            Ok(count) => {
                if count > remaining {
                    bail!("plugin binary comparison exceeds size limit");
                }
                total += count;
                hash.update(&chunk[..count]);
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).context("read conditional binary output"),
        }
    }
    // 2. 【插件写入】【最终撤销】EOF 所在系统调用期间取消也不能返回可用于发布的摘要
    check_cancelled(cancelled)?;
    Ok(hash.finalize().into())
}

#[cfg(test)]
#[path = "tests/revision.rs"]
mod tests;

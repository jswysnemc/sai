use super::paths::Lock;
use anyhow::{bail, Context, Result};
use cap_std::fs::{Dir, OpenOptions};
use flate2::read::GzDecoder;
use sai_plugin_runtime::{
    host::{validate_workspace_path, ArchiveRequest, HttpRequest},
    Capabilities,
};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

const MAX_PAX_METADATA_BYTES: u64 = 128 * 1024;

/// 【插件归档】【临时目录】仅在所有条目通过校验后重命名发布，丢弃时清理暂存内容。
struct Staging {
    parent: Arc<Dir>,
    name: String,
}

impl Drop for Staging {
    /// 【插件归档】【失败清理】取消、下载错误或解压错误都不保留半成品。
    /// @returns 无
    fn drop(&mut self) {
        let _ = self.parent.remove_dir_all(&self.name);
    }
}

struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    /// 【插件归档】【取消信号】宿主 Future 释放时通知有界解压线程停止。
    /// @returns 无
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【插件归档】【下载展开】复用网络授权，展开在线程中进行，发布只发生在等待成功之后。
/// @param directory 目标目录句柄；lease 为目录锁；request 为归档限制；capabilities 为有效授权
/// @returns 新子目录已完整发布时成功
pub(super) async fn extract(
    directory: Arc<Dir>,
    lease: Arc<Lock>,
    request: ArchiveRequest,
    capabilities: Capabilities,
) -> Result<()> {
    validate_workspace_path(&request.destination)?;
    if request.destination == "." || directory.try_exists(&request.destination)? {
        bail!("archive destination must be a new subdirectory");
    }
    if request.max_bytes == 0
        || request.max_bytes > 8 * 1024 * 1024
        || request.max_unpacked_bytes == 0
        || request.max_unpacked_bytes > 64 * 1024 * 1024
        || request.max_entries == 0
        || request.max_entries > 4096
    {
        bail!("plugin archive limits exceed supported bounds");
    }
    let response = crate::plugins::http::send(
        HttpRequest {
            url: request.url.clone(),
            method: "GET".into(),
            headers: Default::default(),
            body: None,
            max_bytes: request.max_bytes,
            timeout_ms: request.timeout_ms,
        },
        capabilities,
        false,
    )
    .await?;
    let response = response
        .error_for_status()
        .map_err(|error| error.without_url())?;
    let bytes = crate::plugins::http::read_bytes(response, request.max_bytes).await?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = CancelOnDrop(cancelled.clone());
    let parent = directory.clone();
    let destination = request.destination.clone();
    let staging = tokio::task::spawn_blocking(move || {
        let _lease = lease;
        unpack(parent, bytes, &request, &cancelled)
    })
    .await
    .context("plugin archive worker stopped")??;
    directory
        .rename(&staging.name, &directory, &destination)
        .context("publish complete plugin archive")?;
    Ok(())
}

/// 【插件归档】【安全解压】拒绝链接、设备、重复路径和越界条目，同时限制总字节与条目数。
/// @param parent 私有目录；bytes 为有界下载；request 为限制；cancelled 为取消标记
/// @returns 尚未发布的暂存目录，错误时自动清理
fn unpack(
    parent: Arc<Dir>,
    bytes: Vec<u8>,
    request: &ArchiveRequest,
    cancelled: &AtomicBool,
) -> Result<Staging> {
    let name = format!(".extract-{}", uuid::Uuid::new_v4());
    parent.create_dir(&name)?;
    let staging = Staging { parent, name };
    let directory = staging.parent.open_dir(&staging.name)?;
    let decoder = GzDecoder::new(bytes.as_slice());
    let overhead = request.max_entries as u64 * 1024 + MAX_PAX_METADATA_BYTES;
    let mut archive = tar::Archive::new(decoder.take(request.max_unpacked_bytes + overhead));
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    let mut metadata_bytes = 0u64;
    let mut entry_count = 0usize;
    for entry in archive.entries()? {
        if cancelled.load(Ordering::Acquire) {
            bail!("plugin archive extraction cancelled");
        }
        let mut entry = entry?;
        entry_count += 1;
        if entry_count > request.max_entries {
            bail!("plugin archive exceeds entry limit");
        }
        let kind = entry.header().entry_type();
        if kind.is_pax_global_extensions() {
            // 【插件归档】【Git 快照】1. 全局 PAX 头只占元数据预算，不生成文件或参与路径去重
            metadata_bytes = metadata_bytes
                .checked_add(entry.size())
                .context("archive metadata size overflow")?;
            if metadata_bytes > MAX_PAX_METADATA_BYTES {
                bail!("plugin archive exceeds metadata byte limit");
            }
            continue;
        }
        let path = entry
            .path()?
            .to_str()
            .context("archive path is not UTF-8")?
            .trim_end_matches('/')
            .to_string();
        validate_workspace_path(&path)?;
        if path == "." || !seen.insert(path.clone()) {
            bail!("archive contains a duplicate or invalid path");
        }
        if kind.is_dir() {
            directory.create_dir_all(&path)?;
            continue;
        }
        if !kind.is_file() {
            bail!("plugin archive contains a link or unsupported file type");
        }
        total = total
            .checked_add(entry.size())
            .context("archive expanded size overflow")?;
        if total > request.max_unpacked_bytes {
            bail!("plugin archive exceeds unpacked byte limit");
        }
        if let Some(parent) = std::path::Path::new(&path)
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            directory.create_dir_all(parent)?;
        }
        let mut file =
            directory.open_with(&path, OpenOptions::new().write(true).create_new(true))?;
        let mut buffer = [0u8; 8192];
        loop {
            if cancelled.load(Ordering::Acquire) {
                bail!("plugin archive extraction cancelled");
            }
            let size = entry.read(&mut buffer)?;
            if size == 0 {
                break;
            }
            file.write_all(&buffer[..size])?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.into_std()
                .set_permissions(std::fs::Permissions::from_mode(
                    entry.header().mode()? & 0o777,
                ))?;
        }
    }
    if seen.is_empty() {
        bail!("plugin archive contains no entries");
    }
    if cancelled.load(Ordering::Acquire) {
        bail!("plugin archive extraction cancelled");
    }
    Ok(staging)
}

#[cfg(test)]
#[path = "tests/archive.rs"]
mod tests;

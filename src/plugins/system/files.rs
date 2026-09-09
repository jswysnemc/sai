use super::paths;
use anyhow::{bail, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use sai_plugin_runtime::host::{
    DirectoryEntry, DirectoryListing, FileInfo, FileReadRequest, FileText, SystemContext,
};
use sai_plugin_runtime::Capabilities;
use std::io::{ErrorKind, Read};

/// 【插件系统】【文件读取】通过授权目录句柄读取普通文件，特殊设备和管道不能阻塞读取入口。
/// @param request 有界读取选项；context 为可信目录；capabilities 为有效授权
/// @returns 文本及是否截断
pub(in crate::plugins) async fn read_text(
    request: FileReadRequest,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<FileText> {
    tokio::task::spawn_blocking(move || {
        let path = paths::authorize(&request.path, &context, &capabilities)?;
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = path
            .directory
            .open_with(&path.relative, &options)
            .context("open plugin file")?;
        if !file.metadata()?.is_file() {
            bail!("plugin text reading requires a regular file");
        }
        let limit = request.max_bytes.clamp(1, 4 * 1024 * 1024);
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        let mut truncated = bytes.len() > limit;
        bytes.truncate(limit);
        // 【插件系统】【文本边界】1. 截断的 UTF-8 尾部不应被误判为整个文件编码错误
        let valid = match std::str::from_utf8(&bytes) {
            Ok(_) => bytes.len(),
            Err(error) if truncated && error.error_len().is_none() => error.valid_up_to(),
            Err(_) if request.lossy => bytes.len(),
            Err(error) => return Err(error).context("plugin file is not UTF-8"),
        };
        bytes.truncate(valid);
        let mut text = if request.lossy {
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            String::from_utf8(bytes)?
        };
        if text.len() > limit {
            let mut end = limit;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text.truncate(end);
            truncated = true;
        }
        Ok(FileText { text, truncated })
    })
    .await
    .context("plugin file reader stopped")?
}

/// 【插件系统】【目录读取】枚举有界条目，越界链接不提供目标属性。
/// @param path 请求目录；max_entries 为条数上限；context 为可信目录；capabilities 为授权
/// @returns 条目数组及是否还有未枚举内容
pub(in crate::plugins) async fn read_directory(
    path: String,
    max_entries: usize,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<DirectoryListing> {
    tokio::task::spawn_blocking(move || {
        let path = paths::authorize(&path, &context, &capabilities)?;
        let directory = path.open_directory()?;
        let limit = max_entries.clamp(1, 1024);
        let mut entries = Vec::new();
        let mut truncated = false;
        for item in directory.entries()? {
            let item = item?;
            if entries.len() == limit {
                truncated = true;
                break;
            }
            let name = item.file_name();
            let metadata = directory.metadata(&name).ok();
            entries.push(DirectoryEntry {
                name: name.to_string_lossy().into_owned(),
                path: path.display.join(&name).to_string_lossy().into_owned(),
                is_dir: metadata.as_ref().is_some_and(|metadata| metadata.is_dir()),
                is_file: metadata.as_ref().is_some_and(|metadata| metadata.is_file()),
            });
        }
        Ok(DirectoryListing { entries, truncated })
    })
    .await
    .context("plugin directory reader stopped")?
}

/// 【插件系统】【路径属性】不存在与未授权分别处理，检查结果不会跟随校验后替换的末级链接。
/// @param path 请求路径；context 为可信目录；capabilities 为授权
/// @returns 普通路径属性，已授权但不存在时返回 None
pub(in crate::plugins) async fn file_info(
    path: String,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<Option<FileInfo>> {
    let result: Result<FileInfo> = tokio::task::spawn_blocking(move || {
        let path = paths::authorize(&path, &context, &capabilities)?;
        let metadata = path.directory.symlink_metadata(&path.relative)?;
        if metadata.file_type().is_symlink() {
            bail!("plugin file path changed to a symbolic link");
        }
        Ok(FileInfo {
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            len: metadata.len(),
        })
    })
    .await
    .context("plugin metadata reader stopped")?;
    match result {
        Ok(info) => Ok(Some(info)),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

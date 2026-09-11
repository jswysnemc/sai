use super::{super::cancel::check_cancelled, directories::Directories};
use anyhow::{bail, Context, Result};
use cap_std::fs::{File, MetadataExt, OpenOptions, OpenOptionsExt};
use std::{
    io::{ErrorKind, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
    sync::atomic::AtomicBool,
};

/// 【插件回收站】【条目预留】保留已原子创建的信息文件，取消仅清理自己创建的条目
pub(super) struct Reservation {
    pub directories: Directories,
    pub name: String,
    info_name: String,
    identity: (u64, u64),
    information: File,
    pub retained: bool,
}

impl Reservation {
    /// 【插件回收站】【信息复核】保持原信息句柄直到提交结束，并检查公开名称仍指向同一文件
    /// @returns 还原信息和标准目录均未被替换时成功
    pub(super) fn check_current(&self) -> Result<()> {
        self.directories.check_current()?;
        let metadata = self
            .directories
            .info
            .symlink_metadata(&self.info_name)
            .context("plugin trash information changed before commit")?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || (metadata.dev(), metadata.ino()) != self.identity
        {
            bail!("plugin trash information changed before commit");
        }
        Ok(())
    }
}

impl Drop for Reservation {
    /// 【插件回收站】【预留清理】源文件尚未移动或已经还原时清理信息，不删除任何回收文件
    /// @returns 无；信息名称被其他程序替换时保持现状
    fn drop(&mut self) {
        if self.retained {
            return;
        }
        if let Ok(metadata) = self.directories.info.symlink_metadata(&self.info_name) {
            if (metadata.dev(), metadata.ino()) == self.identity {
                let _ = self.directories.info.remove_file(&self.info_name);
            }
        }
    }
}

/// 【插件回收站】【还原信息】按规范先独占创建信息文件，再写入原始绝对路径与本地删除时间
/// @param directories 安全目录；original 为原位置；cancelled 为取消标记
/// @returns 尚未移动源文件的预留守卫
pub(super) fn reserve(
    directories: Directories,
    original: &Path,
    cancelled: &AtomicBool,
) -> Result<Reservation> {
    if !original.is_absolute() {
        bail!("plugin trash original path must be absolute");
    }
    let prefix: String = original
        .file_name()
        .context("plugin trash file needs a name")?
        .to_string_lossy()
        .chars()
        .take(40)
        .collect();
    // 1. 【插件回收站】【独占预留】UUID 名称保持有限长度，冲突只尝试新名字，不覆盖已有信息
    for _ in 0..8 {
        check_cancelled(cancelled)?;
        let name = format!("{prefix}-{}", uuid::Uuid::new_v4());
        let info_name = format!("{name}.trashinfo");
        let file = match directories.info.open_with(
            &info_name,
            OpenOptions::new().write(true).create_new(true).mode(0o600),
        ) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error).context("reserve plugin trash information"),
        };
        let metadata = match file.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = directories.info.remove_file(&info_name);
                return Err(error).context("inspect newly created plugin trash information");
            }
        };
        let mut reservation = Reservation {
            directories,
            name,
            info_name,
            identity: (metadata.dev(), metadata.ino()),
            information: file,
            retained: false,
        };
        // 2. 【插件回收站】【原始位置】百分号编码保留路径字节，信息文件必须在文件移动前完整写入
        let path = encode_path(original);
        let date = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
        let content = format!("[Trash Info]\nPath={path}\nDeletionDate={date}\n");
        reservation
            .information
            .write_all(content.as_bytes())
            .context("write plugin trash information")?;
        reservation
            .information
            .sync_all()
            .context("flush plugin trash information")?;
        check_cancelled(cancelled)?;
        return Ok(reservation);
    }
    bail!("unable to reserve a unique plugin trash entry")
}

/// 【插件回收站】【路径编码】按文件系统字节编码各分量，保持斜杠分隔符和原始文件名
/// @param path 原始绝对路径
/// @returns 符合 Trash Info Path 字段要求的文本
fn encode_path(path: &Path) -> String {
    path.as_os_str()
        .as_bytes()
        .split(|byte| *byte == b'/')
        .map(|part| urlencoding::encode_binary(part).into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

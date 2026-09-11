use crate::plugins::system::paths::open_anchor;
use anyhow::{bail, Context, Result};
use cap_std::fs::{Dir, Metadata};
use std::{
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
};

/// 【插件文件】【目标句柄】保存已经授权的父目录与普通文件身份，操作不再解析绝对路径
pub(super) struct Target {
    pub directory: Dir,
    pub name: OsString,
    pub canonical: PathBuf,
    pub metadata: Metadata,
}

impl Target {
    /// 【插件文件】【打开目标】只打开父目录，不读取文件正文，不跟随末级链接
    /// @param canonical 已完成目录授权的绝对目标
    /// @returns 普通文件目标；文件或父目录缺失为 None，特殊对象明确拒绝
    pub(super) fn open(canonical: PathBuf) -> Result<Option<Self>> {
        let parent = canonical
            .parent()
            .context("file removal target has no parent")?;
        let directory = match open_anchor(parent) {
            Ok(directory) => directory,
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == ErrorKind::NotFound) =>
            {
                return Ok(None)
            }
            Err(error) => return Err(error),
        };
        let name = canonical
            .file_name()
            .context("file removal target has no name")?
            .to_os_string();
        let Some(metadata) = inspect(&directory, &name)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            directory,
            name,
            canonical,
            metadata,
        }))
    }

    /// 【插件文件】【提交复核】重新检查普通文件类型及稳定文件标识
    /// @returns 目标仍存在且符合准备时身份为 true，已经缺失为 false
    pub(super) fn check_current(&self) -> Result<bool> {
        let Some(current) = inspect(&self.directory, &self.name)? else {
            return Ok(false);
        };
        self.check_identity(&current)?;
        Ok(true)
    }

    /// 【插件文件】【对象身份】拒绝准备后替换的普通文件，标识不可用时明确失败
    /// @param current 通过不跟随链接检查获得的当前元数据
    /// @returns 对象类型及可用的文件标识一致时成功
    pub(super) fn check_identity(&self, current: &Metadata) -> Result<()> {
        if !current.is_file() || current.file_type().is_symlink() {
            bail!("plugin file removal requires a regular file without links");
        }
        if !same_file(current, &self.metadata)? {
            bail!("plugin file removal target changed before commit");
        }
        Ok(())
    }

    /// 【插件文件】【永久删除】仅解除单个名称的链接，绝不使用递归删除
    /// @returns 删除成功为 true，提交前已缺失为 false
    pub(super) fn remove(&self) -> Result<bool> {
        if !self.check_current()? {
            return Ok(false);
        }
        match self.directory.remove_file(&self.name) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).context("remove authorized plugin file"),
        }
    }
}

/// 【插件文件】【文件标识】以设备和 inode 或 Windows 卷与文件标识比较实际对象
/// @param left 第一个句柄或不跟随链接获得的元数据；right 为第二个对象
/// @returns 标识相同为 true，无法取得标识的平台明确拒绝
pub(super) fn same_file(left: &Metadata, right: &Metadata) -> Result<bool> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok((left.dev(), left.ino()) == (right.dev(), right.ino()))
    }
    #[cfg(windows)]
    {
        use cap_fs_ext::MetadataExt;
        let left = left
            .volume_serial_number()
            .zip(left.file_index())
            .context("plugin file identity is unavailable")?;
        let right = right
            .volume_serial_number()
            .zip(right.file_index())
            .context("plugin file identity is unavailable")?;
        Ok(left == right)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (left, right);
        bail!("plugin file identity is unavailable on this platform")
    }
}

/// 【插件文件】【普通对象】检查末级名称，链接、目录、管道与套接字均不能进入删除流程
/// @param directory 授权父目录；name 为单个文件名称
/// @returns 普通文件元数据或缺失状态
fn inspect(directory: &Dir, name: impl AsRef<Path>) -> Result<Option<Metadata>> {
    match directory.symlink_metadata(name) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(Some(metadata))
        }
        Ok(_) => bail!("plugin file removal requires a regular file without links"),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("inspect authorized plugin removal target"),
    }
}

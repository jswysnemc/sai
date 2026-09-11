use super::super::{cancel::check_cancelled, target::Target};
use super::{
    directories,
    metadata::{self, Reservation},
};
use anyhow::{bail, Context, Result};
use cap_std::fs::Dir;
use std::{
    ffi::{CString, OsStr},
    io::ErrorKind,
    os::{fd::AsRawFd, unix::ffi::OsStrExt},
    path::Path,
    sync::atomic::AtomicBool,
};

/// 【插件回收站】【待移动条目】持有标准还原信息和目标目录，不持有或复制文件正文
pub(in crate::plugins::file_ops) struct Entry {
    reservation: Reservation,
}

/// 【插件回收站】【正式准备】通过可信环境取得标准用户回收站，不使用应用私有备份代替
/// @param target 已授权普通文件；cancelled 为取消标记
/// @returns 已预留还原信息的条目
pub(in crate::plugins::file_ops) fn prepare(
    target: &Target,
    cancelled: &AtomicBool,
) -> Result<Entry> {
    prepare_at(target, &directories::home()?, cancelled)
}

/// 【插件回收站】【目录组合】将目录安全检查与还原信息预留组合为无源文件变更的准备阶段
/// @param target 已授权文件；data_home 为可信数据目录；cancelled 为撤销标记
/// @returns 待提交条目
fn prepare_at(target: &Target, data_home: &Path, cancelled: &AtomicBool) -> Result<Entry> {
    let directories = directories::open(data_home, target, cancelled)?;
    let reservation = metadata::reserve(directories, &target.canonical, cancelled)?;
    check_cancelled(cancelled)?;
    Ok(Entry { reservation })
}

impl Entry {
    /// 【插件回收站】【提交移动】复核源文件后执行禁止覆盖的目录句柄重命名
    /// @param target 授权目标及准备时文件身份
    /// @returns 成功移动为 true，提交前已经缺失为 false
    pub(in crate::plugins::file_ops) fn commit(&mut self, target: &Target) -> Result<bool> {
        if !target.check_current()? {
            return Ok(false);
        }
        self.reservation.check_current()?;
        self.move_and_verify(target)
    }

    /// 【插件回收站】【移动后复核】检查实际移入对象，外部替换只能导致安全还原或保留可恢复条目
    /// @param target 已在提交入口完成复核的文件
    /// @returns 普通文件移动结果，错误不删除回收站中的对象
    fn move_and_verify(&mut self, target: &Target) -> Result<bool> {
        // 1. 【插件回收站】【原子移动】源和目标均相对固定目录，目标冲突不得覆盖，也不得跨设备复制
        match rename_noreplace(
            &target.directory,
            &target.name,
            &self.reservation.directories.files,
            OsStr::new(&self.reservation.name),
        ) {
            Ok(()) => self.reservation.retained = true,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(error).context("move authorized plugin file into the system trash")
            }
        }
        let checked = self
            .reservation
            .directories
            .files
            .symlink_metadata(&self.reservation.name)
            .context("inspect moved plugin trash entry")
            .and_then(|metadata| target.check_identity(&metadata))
            .and_then(|()| self.reservation.check_current());
        if let Err(error) = checked {
            return self.restore_after_change(target, error);
        }
        Ok(true)
    }

    /// 【插件回收站】【变化恢复】移动期间出现替换时优先还原，原位置冲突则保留完整回收条目
    /// @param target 原位置；error 为移动后的身份或类型检查错误
    /// @returns 始终报错，并明确说明还原结果，不覆盖冲突文件
    fn restore_after_change(&mut self, target: &Target, error: anyhow::Error) -> Result<bool> {
        // 1. 【插件回收站】【无覆盖还原】只移动捕获到的对象，不遍历目录或跟随链接
        match rename_noreplace(
            &self.reservation.directories.files,
            OsStr::new(&self.reservation.name),
            &target.directory,
            &target.name,
        ) {
            Ok(()) => {
                self.reservation.retained = false;
                Err(error).context("plugin trash source changed during commit; entry restored")
            }
            Err(restore) => {
                let path = self
                    .reservation
                    .directories
                    .canonical
                    .join("files")
                    .join(&self.reservation.name);
                bail!("plugin trash source changed during commit; item retained in system trash at {}: {error:#}; restore failed: {restore}", path.display())
            }
        }
    }
}

/// 【插件回收站】【无覆盖重命名】使用 Linux renameat2 的原子禁止覆盖标志
/// @param from 源父目录；name 为源名称；to 为目标目录；destination 为目标名称
/// @returns 单次移动结果，内核或文件系统不支持时明确失败
fn rename_noreplace(
    from: &Dir,
    name: &OsStr,
    to: &Dir,
    destination: &OsStr,
) -> std::io::Result<()> {
    let name = CString::new(name.as_bytes())?;
    let destination = CString::new(destination.as_bytes())?;
    // 1. 【插件回收站】【系统调用】借用期间两个目录句柄与两个 NUL 结尾名称均保持有效
    let result = unsafe {
        libc::renameat2(
            from.as_raw_fd(),
            name.as_ptr(),
            to.as_raw_fd(),
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(test)]
#[path = "tests/changes.rs"]
mod change_tests;
#[cfg(test)]
#[path = "tests/directories.rs"]
mod directory_tests;
#[cfg(test)]
#[path = "tests/entry.rs"]
mod entry_tests;
#[cfg(test)]
#[path = "tests/support.rs"]
mod test_support;

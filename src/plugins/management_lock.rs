use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};

/// 【插件】【管理锁】拥有一次管理操作的互斥权限，不把锁的释放依赖于全部文件描述符关闭。
pub(super) struct ManagementLock(File);

/// 【插件】【管理互斥】锁住配置目录，避免并发覆盖配置或交错移动安装目录。
/// @param paths 当前实例的应用路径
/// @returns 随作用域显式解锁的守卫；已有管理操作时立即返回错误
pub(super) fn acquire(paths: &SaiPaths) -> Result<ManagementLock> {
    std::fs::create_dir_all(&paths.config_dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(paths.config_dir.join(".plugins.lock"))?;
    file.try_lock()
        .context("another plugin management command is in progress")?;
    Ok(ManagementLock(file))
}

impl Drop for ManagementLock {
    /// 【插件】【管理结束】明确释放共享文件描述上的锁，避免 fork 子进程在 exec 前继续持锁。
    /// @returns 无；文件句柄随后按正常生命周期关闭
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

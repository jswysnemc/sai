use anyhow::{Context, Result};
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use std::path::{Path, PathBuf};

/// 【插件私有数据】【目录锁】持有目录操作的文件锁，离开作用域时显式释放。
pub(super) struct Lock(std::fs::File);

impl Drop for Lock {
    /// 【插件私有数据】【释放锁】取消或普通返回都释放锁，不依赖子进程描述符关闭。
    /// @returns 无
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// 【插件私有数据】【目录根】打开宿主指定路径，后续子目录不跟随符号链接。
/// @param base 宿主应用目录
/// @returns 目录句柄与规范显示路径
pub(super) fn root(base: &Path) -> Result<(Dir, PathBuf)> {
    std::fs::create_dir_all(base)?;
    let path = dunce::canonicalize(base)?;
    Ok((Dir::open_ambient_dir(&path, ambient_authority())?, path))
}

/// 【插件私有数据】【命名空间】用摘要隔离插件与会话，空会话只属于直接命令入口。
/// @param base 应用根目录；category 为数据类别；plugin 为绑定插件；session 为可信会话
/// @returns 隔离目录及显示路径
pub(super) fn namespace(
    base: &Path,
    category: &str,
    plugin: &str,
    session: &str,
) -> Result<(Dir, PathBuf)> {
    let (mut directory, mut display) = root(base)?;
    for name in [category.to_string(), hash(plugin), hash(session)] {
        match directory.create_dir(&name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        directory = directory
            .open_dir_nofollow(&name)
            .context("open private namespace without following links")?;
        display.push(name);
    }
    Ok((directory, display))
}

/// 【插件私有数据】【互斥】以不跟随链接的文件句柄取得立即失败的操作锁。
/// @param directory 可信目录句柄；name 为锁文件名
/// @returns 锁守卫；并发占用时提示重试
pub(super) fn lock(directory: &Dir, name: &str) -> Result<Lock> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .follow(FollowSymlinks::No);
    let file = directory.open_with(name, &options)?.into_std();
    file.try_lock()
        .context("plugin private data is busy; retry the operation")?;
    Ok(Lock(file))
}

/// 【插件私有数据】【键摘要】任意合法键只产生固定 ASCII 文件名。
/// @param value 插件、会话或私有键
/// @returns 64 位十六进制摘要文本
pub(super) fn hash(value: &str) -> String {
    blake3::hash(value.as_bytes()).to_hex().to_string()
}

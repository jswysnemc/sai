use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

/// 【Web 分发】【指针读取】读取期间持有共享锁，避免 Windows 替换仍在读取的文件
/// 参数: root 为缓存目录；返回: 完整安装目录名，旧缓存没有锁文件时仍可读取
pub(super) fn read(root: &Path) -> Result<String> {
    let lock = match std::fs::File::open(root.join(".current.lock")) {
        Ok(lock) => Some(lock),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(lock) = &lock {
        lock.lock_shared().context("read Web asset pointer lock")?;
    }
    Ok(std::fs::read_to_string(root.join("current"))?)
}

/// 【Web 分发】【指针切换】跨进程串行写入完整版本指针，失败时保留原指针
/// 参数: root 为缓存目录，installation 为完整安装目录名；返回: 原子更新结果
pub(super) fn replace(root: &Path, installation: &str) -> Result<()> {
    // 1. 【Web 分发】【更新锁定】使用独立锁文件，避免 Windows 并发替换同一个目标文件
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".current.lock"))?;
    lock.lock().context("lock Web asset pointer")?;

    // 2. 【Web 分发】【完整写入】先持久化临时文件，不直接修改现有版本指针
    let mut pointer = tempfile::NamedTempFile::new_in(root)?;
    pointer.write_all(installation.as_bytes())?;
    pointer.as_file().sync_all()?;

    // 3. 【Web 分发】【原子替换】关闭临时句柄后替换指针，锁随文件句柄释放
    pointer
        .into_temp_path()
        .persist(root.join("current"))
        .context("replace Web asset pointer")?;
    Ok(())
}

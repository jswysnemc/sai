//! 编辑工具共用的进程间锁；锁文件独立于目标 inode，原子替换后仍保持互斥。

use anyhow::{Context, Result, bail};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(crate) struct FileEditLocks {
    _files: Vec<File>,
}

impl FileEditLocks {
    /// 【文件编辑】【并发加锁】规范化路径并按固定顺序取得互斥锁，覆盖读改写全过程。
    /// 参数: paths 为全部源和目标路径；返回: 持有期间有效的锁集合，十秒内无法取得则失败。
    pub(crate) fn acquire(paths: &[PathBuf]) -> Result<Self> {
        Self::acquire_for(paths, Duration::from_secs(10))
    }

    /// 【文件编辑】【有界等待】使用统一截止时间，防止多文件竞争累计无限等待。
    /// 参数: paths 为目标集合，timeout 为等待上限；返回: 锁守卫。
    pub(crate) fn acquire_for(paths: &[PathBuf], timeout: Duration) -> Result<Self> {
        let root = directories::BaseDirs::new()
            .context("cannot locate user cache directory")?
            .cache_dir()
            .join("sai/file-edit-locks");
        std::fs::create_dir_all(&root)?;
        let mut keys = paths
            .iter()
            .map(|path| lock_key(path))
            .collect::<Result<Vec<_>>>()?;
        keys.sort();
        keys.dedup();
        let deadline = Instant::now() + timeout;
        let mut files = Vec::new();
        for key in keys {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join(key))?;
            loop {
                super::ensure_active()?;
                match file.try_lock() {
                    Ok(()) => break,
                    Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(TryLockError::WouldBlock) => bail!(
                        "file edit conflict: another edit holds a target lock; retry after it finishes"
                    ),
                    Err(TryLockError::Error(error)) => return Err(error.into()),
                }
            }
            files.push(file);
        }
        Ok(Self { _files: files })
    }
}

/// 【文件编辑】【锁标识】解析已存在的祖先，使相对路径、符号链接别名和新文件共享锁。
/// 参数: path 为文件路径；返回: 不泄漏文件名的稳定锁键。
fn lock_key(path: &Path) -> Result<String> {
    let mut ancestor = path;
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .context("invalid edit path")?
                .to_os_string(),
        );
        ancestor = ancestor
            .parent()
            .context("edit path has no existing ancestor")?;
    }
    let mut canonical = ancestor.canonicalize()?;
    for component in missing.into_iter().rev() {
        canonical.push(component);
    }
    #[cfg(windows)]
    let canonical = canonical.to_string_lossy().to_lowercase();
    #[cfg(windows)]
    let bytes = canonical.as_bytes();
    #[cfg(not(windows))]
    let bytes = canonical.as_os_str().as_encoded_bytes();
    Ok(blake3::hash(bytes).to_hex().to_string())
}

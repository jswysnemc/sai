use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

type PathLock = Arc<Mutex<()>>;

static FILE_WRITE_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();

/// 在指定文件路径的进程内互斥锁中执行保存操作。
///
/// 参数:
/// - `path`: 需要串行化写入的规范路径
/// - `operation`: 持有路径锁期间执行的文件操作
///
/// 返回:
/// - 文件操作结果
pub(super) fn with_file_write_lock<T>(
    path: &Path,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let path_lock = path_lock(path)?;
    let _guard = path_lock
        .lock()
        .map_err(|_| anyhow!("workspace file write lock is poisoned"))?;
    operation()
}

/// 获取指定路径对应的共享互斥锁。
///
/// 参数:
/// - `path`: 需要串行化写入的规范路径
///
/// 返回:
/// - 路径对应的共享互斥锁
fn path_lock(path: &Path) -> Result<PathLock> {
    let registry = FILE_WRITE_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = registry
        .lock()
        .map_err(|_| anyhow!("workspace file write lock registry is poisoned"))?;
    locks.retain(|_, lock| lock.strong_count() > 0);
    if let Some(lock) = locks.get(path).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(path.to_path_buf(), Arc::downgrade(&lock));
    Ok(lock)
}

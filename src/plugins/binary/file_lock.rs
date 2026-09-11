use super::files::check_cancelled;
use crate::plugins::private::paths;
use anyhow::{bail, Context, Result};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::OpenOptions;
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

const NAME: &str = "plugin-binary-write.lock";

/// 【插件写入】【应用互斥】同一应用状态目录中的所有正式二进制输出共用一个稳定文件锁
pub(super) struct Lock {
    file: std::fs::File,
    path: PathBuf,
}

impl Lock {
    /// 【插件写入】【锁文件保护】禁止输出替换正在协调同一应用写入的锁文件
    /// @param target 已规范化的目标路径
    /// @returns 输出不是锁文件本身时成功
    pub(super) fn check_target(&self, target: &Path) -> Result<()> {
        if target == self.path {
            bail!("plugin binary output cannot replace its coordination lock");
        }
        Ok(())
    }
}

impl Drop for Lock {
    /// 【插件写入】【释放互斥】暂存清理和发布完成后释放文件锁
    /// @returns 无；不会删除锁文件，避免其他进程锁住不同 inode
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

/// 【插件写入】【有界等待】安全打开固定锁文件，在阻塞线程中短暂等待并持续检查取消
/// @param state_dir 宿主应用状态目录；cancelled 为调用取消标记
/// @returns 持有跨进程互斥的守卫；等待最多十分钟，运行时可施加更短时限
pub(super) fn acquire(state_dir: &Path, cancelled: &AtomicBool) -> Result<Lock> {
    // 1. 【插件写入】【锁句柄】固定名称只接受普通文件，打开时不跟随链接或等待管道另一端
    check_cancelled(cancelled)?;
    let (directory, display) = paths::root(state_dir)?;
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = directory
        .open_with(NAME, &options)
        .context("open plugin binary write lock")?
        .into_std();
    if !file.metadata()?.is_file() {
        bail!("plugin binary write lock requires a regular file");
    }
    // 2. 【插件写入】【锁等待】正常竞争短暂等待，撤销信号和硬时限均能终止等待
    let started = Instant::now();
    loop {
        check_cancelled(cancelled)?;
        match file.try_lock() {
            Ok(()) => {
                let path = std::fs::canonicalize(display.join(NAME))
                    .context("resolve plugin binary lock file")?;
                let lock = Lock { file, path };
                check_cancelled(cancelled)?;
                return Ok(lock);
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                if started.elapsed() >= Duration::from_secs(600) {
                    bail!("plugin binary write lock timed out");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error).context("lock plugin binary output"),
        }
    }
}

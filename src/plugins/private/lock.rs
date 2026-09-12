use super::paths;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{
    host::{validate_storage_key, PluginLock},
    Capabilities,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

impl PluginLock for paths::Lock {}
struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    /// 【插件互斥】【取消等待】无参数；丢弃 Future 时通知阻塞线程停止取得锁
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// 【插件互斥】【正式宿主】锁键只进入绑定插件的私有命名空间
/// @param state_dir 可信状态目录；id 为插件；key 为私有键；timeout_ms 为等待上限；capabilities 为授权
/// @returns 自动释放的锁租约，失败和取消不留下被占用的锁
pub(super) async fn acquire(
    state_dir: PathBuf,
    id: String,
    key: String,
    timeout_ms: u64,
    capabilities: &Capabilities,
) -> Result<Box<dyn PluginLock>> {
    if !capabilities.system.plugin_storage {
        bail!("plugin storage is not allowed");
    }
    validate_storage_key(&key)?;
    anyhow::ensure!(
        (1..=600_000).contains(&timeout_ms),
        "plugin lock timeout must be 1-600000 milliseconds"
    );
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel = Cancel(cancelled.clone());
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let lock = tokio::task::spawn_blocking(move || {
        // 1. 【插件互斥】【有界目录】注册新键使用短目录锁，持久文件最多 128 项
        let (directory, _) = paths::namespace(&state_dir, "plugin-locks", &id, "")?;
        let name = format!("{}.lock", paths::hash(&key));
        let catalog = wait(&directory, ".catalog.lock", deadline, &cancelled)?;
        if !directory.try_exists(&name)? && directory.entries()?.take(129).count() >= 129 {
            bail!("plugin locks exceed 128 keys");
        }
        // 2. 【插件互斥】【稳定文件】第一次取得锁同时创建固定 inode；已有锁不删除也不替换
        let initial = paths::lock(&directory, &name);
        drop(catalog);
        let lock = match initial {
            Ok(lock) => lock,
            Err(error) if busy(&error) => wait(&directory, &name, deadline, &cancelled)?,
            Err(error) => return Err(error),
        };
        check(&cancelled)?;
        Ok::<_, anyhow::Error>(lock)
    })
    .await
    .context("plugin lock worker stopped")??;
    Ok(Box::new(lock))
}

/// 【插件互斥】【占用识别】只把系统明确的锁占用视为可重试
/// @param error 完整宿主错误
/// @returns 是否由锁占用引起
fn busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<std::fs::TryLockError>(),
            Some(std::fs::TryLockError::WouldBlock)
        )
    })
}

/// 【插件互斥】【撤销检查】每次重试及返回前核对取消状态
/// @param cancelled 本次调用标记
/// @returns 尚未取消时成功
fn check(cancelled: &AtomicBool) -> Result<()> {
    anyhow::ensure!(
        !cancelled.load(Ordering::Acquire),
        "plugin lock acquisition cancelled"
    );
    Ok(())
}

/// 【插件互斥】【有界等待】短间隔等待固定锁，权限错误立即返回
/// @param directory 可信目录；name 为固定文件名；deadline 为截止时间；cancelled 为取消状态
/// @returns 锁守卫或明确超时错误
fn wait(
    directory: &cap_std::fs::Dir,
    name: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<paths::Lock> {
    loop {
        check(cancelled)?;
        match paths::lock(directory, name) {
            Ok(lock) => {
                check(cancelled)?;
                return Ok(lock);
            }
            Err(error) if busy(&error) => {
                if Instant::now() >= deadline {
                    bail!("plugin lock acquisition timed out");
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(error) => return Err(error),
        }
    }
}

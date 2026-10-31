use super::client::WeixinBotClient;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize, Deserialize)]
struct RuntimeIdentity {
    pid: u32,
    fingerprint: String,
}

/// 【微信网关】【运行租约】持有操作系统文件锁；进程退出后自动释放，不依赖 PID 文件清理。
pub(crate) struct RuntimeGuard {
    _lock: File,
    identity: PathBuf,
}

impl RuntimeGuard {
    /// 【微信网关】【运行登记】参数 paths/client 为状态路径与账号客户端，返回唯一运行租约。
    pub(crate) fn acquire(paths: &SaiPaths, client: &WeixinBotClient) -> Result<Self> {
        let lock = lock_file(paths, "weixin-runtime.lock")?;
        lock.try_lock()
            .context("Weixin gateway already running; refusing a duplicate long poll")?;
        let identity = paths.state_dir.join("gateways/weixin-runtime.json");
        let record = RuntimeIdentity {
            pid: std::process::id(),
            fingerprint: client.fingerprint(),
        };
        let mut temporary = tempfile::NamedTempFile::new_in(identity.parent().unwrap())?;
        serde_json::to_writer(temporary.as_file_mut(), &record)?;
        temporary.persist(&identity).map_err(|error| error.error)?;
        Ok(Self {
            _lock: lock,
            identity,
        })
    }
}

impl Drop for RuntimeGuard {
    /// 【微信网关】【运行清理】无参数，返回无；先移除状态，再释放运行锁。
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.identity);
    }
}

/// 【微信网关】【状态检查】参数 paths/client 为路径和账号，返回同账号网关是否持有运行锁。
pub(crate) fn is_running(paths: &SaiPaths, client: &WeixinBotClient) -> Result<bool> {
    let lock = lock_file(paths, "weixin-runtime.lock")?;
    match lock.try_lock() {
        Ok(()) => return Ok(false),
        Err(std::fs::TryLockError::WouldBlock) => {}
        Err(error) => return Err(error.into()),
    }
    let identity = paths.state_dir.join("gateways/weixin-runtime.json");
    let record: RuntimeIdentity = match std::fs::read(&identity) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if record.fingerprint != client.fingerprint() {
        bail!("Weixin gateway is running with different credentials; restart it with the current account before sending");
    }
    Ok(crate::runner::process_liveness::process_exists(record.pid))
}

/// 【微信网关】【启动锁】参数 paths 为状态路径，返回跨进程启动锁；超时不阻塞执行线程。
pub(crate) async fn startup_lock(paths: &SaiPaths) -> Result<File> {
    let lock = lock_file(paths, "weixin-start.lock")?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            match lock.try_lock() {
                Ok(()) => return Ok(lock),
                Err(std::fs::TryLockError::WouldBlock) => {
                    tokio::time::sleep(Duration::from_millis(50)).await
                }
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await
    .context("timed out waiting for Weixin gateway startup lock")?
}

/// 【微信网关】【锁文件】参数 paths/name 为状态路径与固定文件名，返回不截断的锁文件。
fn lock_file(paths: &SaiPaths, name: &str) -> Result<File> {
    let directory = paths.state_dir.join("gateways");
    std::fs::create_dir_all(&directory)?;
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(name))?)
}

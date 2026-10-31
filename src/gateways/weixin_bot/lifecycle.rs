use super::{client::WeixinBotClient, runtime};
use crate::config::AppConfig;
use crate::gateways::manager::{self, ManagedGateway};
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::future::Future;
use std::time::Duration;

/// 【微信网关】【按需启动】参数 paths/config/client 为运行路径、配置及账号，返回启动确认结果。
/// 不自动登录、不写回启用偏好；后台进程继续使用已有网关管理与日志路径
pub(crate) async fn ensure_running(
    paths: &SaiPaths,
    config: &AppConfig,
    client: &WeixinBotClient,
) -> Result<()> {
    ensure_with(paths, client, || async {
        manager::start_gateway_unlocked(paths, config, ManagedGateway::Weixin).await?;
        Ok(())
    })
    .await
}

/// 【微信网关】【启动协调】参数 paths/client/start 为路径、账号及启动操作，返回唯一启动确认结果。
async fn ensure_with<F: Future<Output = Result<()>>>(
    paths: &SaiPaths,
    client: &WeixinBotClient,
    start: impl FnOnce() -> F,
) -> Result<()> {
    if runtime::is_running(paths, client)? {
        return Ok(());
    }
    let _startup = runtime::startup_lock(paths).await?;
    // 1. 【微信网关】【并发复查】拿到启动锁后重查，其他终端可能已经启动网关
    if runtime::is_running(paths, client)? {
        return Ok(());
    }
    start().await.context(
        "failed to start Weixin gateway; check the gateway stderr log and login configuration",
    )?;
    // 2. 【微信网关】【启动确认】仅持有运行锁且账号一致才允许发送，不能把 shell 启动当成服务就绪
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if runtime::is_running(paths, client)? { return Ok::<_, anyhow::Error>(()); }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }).await.context("Weixin gateway did not become ready; check gateways/logs and ensure the running binary supports gateway startup detection")?
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;

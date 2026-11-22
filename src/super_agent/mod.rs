pub mod cards;
pub mod channel_bridge;
pub mod gateway_bridge;
pub mod leader_runtime;
pub mod model;
pub mod policy;
pub mod prompts;
pub mod service;
pub mod store;
pub mod tools;
pub mod watchdog;
pub mod worker_executor;

#[cfg(test)]
mod tests;

use crate::paths::SaiPaths;
use anyhow::Result;
pub use service::{SpawnRunnerRequest, SuperAgentService};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 【超级智能体】【进程单例】按 state_dir 复用服务实例，确保同进程内 TUI/Web/Gateway 共享实时事件与活跃 Runner 句柄。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 共享的 Super-Agent 服务句柄
pub fn shared_service(paths: &SaiPaths) -> Result<Arc<SuperAgentService>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<SuperAgentService>>>> = OnceLock::new();
    let map = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let key = paths.state_dir.display().to_string();
    let mut guard = map.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(existing) = guard.get(&key) {
        return Ok(Arc::clone(existing));
    }
    let service = SuperAgentService::open(paths)?;
    guard.insert(key, Arc::clone(&service));
    Ok(service)
}

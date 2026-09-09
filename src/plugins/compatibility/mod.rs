mod linux_game;
mod web_search;

use crate::config::AppConfig;
use anyhow::Result;
use sai_plugin_runtime::Capabilities;
use serde_json::Value;

/// 【插件兼容】【运行快照】旧配置的派生值仅供执行，不进入 plugins.jsonc。
#[derive(Clone, Debug)]
pub(super) struct RuntimeOverrides {
    pub settings: Value,
    pub capabilities: Capabilities,
}

/// 【插件兼容】【定向迁移】只向相应内置包交付它原来拥有的设置。
/// @param id 内置包 ID；config 为主配置；settings 为显式插件设置；declared 为原始清单能力
/// @returns 必要的运行时覆盖；没有兼容字段的包返回 None
pub(super) fn resolve(
    id: &str,
    config: &AppConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<Option<RuntimeOverrides>> {
    match id {
        "web-search" => web_search::resolve(&config.plugins.web, settings, declared).map(Some),
        "linux-game-investigation" => linux_game::resolve(config, settings, declared).map(Some),
        _ => Ok(None),
    }
}

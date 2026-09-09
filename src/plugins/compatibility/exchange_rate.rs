use super::RuntimeOverrides;
use crate::config::ExchangeRatePluginConfig;
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【汇率兼容】【运行设置】定向传递旧密钥和免费回退开关，显式插件设置按字段覆盖。
/// @param legacy 旧汇率配置；settings 为显式插件设置；declared 为静态能力声明
/// @returns 不修改主配置或插件配置文件的运行时设置快照
pub(super) fn resolve(
    legacy: &ExchangeRatePluginConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mut merged = json!({
        "api_key":legacy.api_key,
        "free_fallback_enabled":legacy.free_fallback_enabled,
    });
    let object = merged.as_object_mut().expect("constructed settings object");
    for (name, value) in settings
        .as_object()
        .context("exchange-rate settings must be an object")?
    {
        object.insert(name.clone(), value.clone());
    }
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities: declared.clone(),
    })
}

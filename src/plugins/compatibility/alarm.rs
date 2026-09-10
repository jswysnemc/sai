use super::RuntimeOverrides;
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde::Deserialize;
use serde_json::Value;

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    audio_paths: Option<Vec<String>>,
}

/// 【闹钟兼容】【读取声明】把显式音频路径设置转换成受限读取声明，不改变已有显式授权。
/// @param settings 用户保存的插件设置；declared 为原清单能力
/// @returns 本次加载使用的设置和能力
pub(super) fn resolve(settings: &Value, declared: &Capabilities) -> Result<RuntimeOverrides> {
    let config: Settings =
        serde_json::from_value(settings.clone()).context("invalid alarm settings")?;
    let mut capabilities = declared.clone();
    if let Some(paths) = config.audio_paths {
        anyhow::ensure!(paths.len() <= 64, "alarm audio_paths exceeds 64 paths");
        capabilities.system.read_paths = paths.into_iter().collect();
    }
    capabilities.validate()?;
    Ok(RuntimeOverrides {
        settings: settings.clone(),
        capabilities,
    })
}

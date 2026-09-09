use super::RuntimeOverrides;
use crate::config::AppConfig;
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【插件兼容】【游戏调查】旧预算和显示设置仅作为内置包默认值，显式插件设置优先。
/// @param config 当前应用配置；settings 为用户保存的插件设置；declared 为包声明
/// @returns 不包含供应商或凭据的运行快照
pub(super) fn resolve(
    config: &AppConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mode = match config
        .display
        .tool_calls
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "hidden" => "hidden",
        "full" => "full",
        _ => "summary",
    };
    let mut derived = json!({
        "max_tool_steps": config.plugins.linux_game_compatibility.max_tool_steps,
        "progress_mode": mode,
        "language": if crate::i18n::is_zh() { "zh" } else { "en" },
    });
    derived
        .as_object_mut()
        .expect("settings defaults are an object")
        .extend(
            settings
                .as_object()
                .context("plugin settings must be an object")?
                .clone(),
        );
    Ok(RuntimeOverrides {
        settings: derived,
        capabilities: declared.clone(),
    })
}

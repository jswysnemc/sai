use super::RuntimeOverrides;
use crate::config::AppConfig;
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【插件兼容】【诊断配置】只派生命令时限和输出字符限制，显式插件设置优先。
/// @param config 旧应用配置；settings 为插件设置；declared 为包能力
/// @returns 不修改保存配置的运行快照
pub(super) fn resolve(
    config: &AppConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let legacy = &config.plugins.diagnostics;
    let mut derived = json!({
        "command_timeout_ms":legacy.command_timeout_seconds.clamp(1, 120) * 1000,
        "max_stdout_chars":legacy.max_stdout_chars.min(200_000),
        "max_stderr_chars":legacy.max_stderr_chars.min(200_000),
    });
    derived
        .as_object_mut()
        .expect("diagnostic settings are an object")
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

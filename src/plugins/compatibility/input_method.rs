use super::RuntimeOverrides;
use crate::config::AppConfig;
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【插件兼容】【输入法调查】仅传递原调查实际使用的业务设置，供应商选择由调用服务绑定。
/// @param config 当前应用配置；settings 为显式插件设置；declared 为包声明
/// @returns 不包含模型地址、凭据或无效旧字段的运行快照
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
        "max_tool_steps": config.plugins.deep_diagnose.max_tool_steps,
        "tool_timeout_ms": config.plugins.deep_diagnose.tool_call_timeout_seconds
            .max(5).saturating_mul(1000).min(900_000),
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

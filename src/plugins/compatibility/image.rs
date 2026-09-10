use super::RuntimeOverrides;
use crate::config::{ImageGenerationPluginConfig, PrintImagePluginConfig};
use anyhow::{Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【图片兼容】【生成设置】旧设置仅作为内置包默认值，显式设置逐字段覆盖。
/// @param legacy 原生成设置；settings 为显式插件设置；declared 为包能力
/// @returns 不写回磁盘的设置、精确 API 来源和输出目录声明
pub(super) fn generation(
    legacy: &ImageGenerationPluginConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let merged = merge(serde_json::to_value(legacy)?, settings)?;
    let base = merged
        .get("base_url")
        .and_then(Value::as_str)
        .context("image-generation.base_url must be a string")?
        .trim();
    let output = merged
        .get("output_dir")
        .and_then(Value::as_str)
        .context("image-generation.output_dir must be a string")?
        .trim();
    let mut capabilities = declared.clone();
    // 1. 【图片兼容】【配置授权】原地址和目录替换包内默认值，不因历史默认值保留多余授权
    capabilities.http.clear();
    if !base.is_empty() {
        let url = reqwest::Url::parse(base).context("invalid image-generation base URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            anyhow::bail!("image-generation base URL must use HTTP(S) without credentials");
        }
        capabilities.http.insert(url.origin().ascii_serialization());
    }
    capabilities.binary.write_paths = [if output.is_empty() {
        ".".to_string()
    } else {
        output.to_string()
    }]
    .into();
    capabilities.validate()?;
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities,
    })
}

/// 【图片兼容】【显示设置】保留原百分比默认值，语言仅提供给显示包。
/// @param legacy 原显示设置；settings 为显式插件设置；declared 为展示能力
/// @returns 按字段覆盖后的内存设置
pub(super) fn display(
    legacy: &PrintImagePluginConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let defaults = json!({"width_percent":legacy.width_percent, "height_percent":legacy.height_percent,
        "language":if crate::i18n::is_zh() { "zh" } else { "en" }});
    Ok(RuntimeOverrides {
        settings: merge(defaults, settings)?,
        capabilities: declared.clone(),
    })
}

/// 【图片兼容】【字段合并】启用状态由插件管理维护，不将旧 enabled 写入业务设置。
/// @param legacy 默认对象；settings 为显式对象
/// @returns 保留显式 false 的合并结果
fn merge(mut legacy: Value, settings: &Value) -> Result<Value> {
    let object = legacy
        .as_object_mut()
        .context("image defaults must be an object")?;
    object.remove("enabled");
    for (key, value) in settings
        .as_object()
        .context("image plugin settings must be an object")?
    {
        object.insert(key.clone(), value.clone());
    }
    Ok(legacy)
}

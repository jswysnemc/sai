use super::RuntimeOverrides;
use crate::{config::AppConfig, paths::SaiPaths};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【搜图兼容】【设置快照】旧设置和图片目录仅作为内置包默认值，不向外部包交付主配置。
/// @param config 原主配置；paths 为应用路径；settings 为显式设置；declared 为包声明
/// @returns 当前设置与精确搜索来源、输出目录声明，不写回管理配置
pub(super) fn resolve(
    config: &AppConfig,
    paths: &SaiPaths,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mut merged = serde_json::to_value(&config.plugins.web_images)?;
    let object = merged
        .as_object_mut()
        .context("invalid web image defaults")?;
    object.remove("enabled");
    object.insert(
        "cache_dir".into(),
        json!(paths.pictures_dir.join("web-images")),
    );
    object.insert(
        "language".into(),
        json!(if crate::i18n::is_zh() { "zh" } else { "en" }),
    );
    object.insert(
        "duckduckgo_base_url".into(),
        json!("https://duckduckgo.com"),
    );
    object.insert("bing_base_url".into(), json!("https://www.bing.com"));
    for (key, value) in settings
        .as_object()
        .context("web-images settings must be an object")?
    {
        object.insert(key.clone(), value.clone());
    }
    let mut capabilities = declared.clone();
    capabilities.http.clear();
    for key in ["duckduckgo_base_url", "bing_base_url"] {
        let value = merged[key]
            .as_str()
            .context("web-images search URL must be a string")?
            .trim();
        let url = reqwest::Url::parse(value).context("invalid web-images search URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!("web-images search URL must use HTTP(S) without credentials, query or fragment");
        }
        capabilities.http.insert(url.origin().ascii_serialization());
    }
    // 1. 【搜图兼容】【目录授权】显式授权仍与当前目录声明求交集，改变目录不会扩大已有授权
    let cache = merged["cache_dir"]
        .as_str()
        .context("web-images.cache_dir must be a string")?
        .trim();
    capabilities.binary.write_paths = [cache.to_string()].into();
    capabilities.validate()?;
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities,
    })
}

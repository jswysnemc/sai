use super::RuntimeOverrides;
use crate::config::WebSearchConfig;
use anyhow::{bail, Context, Result};
use reqwest::Url;
use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【搜索兼容】【运行设置】合并旧字段和显式设置，定向解析原有凭据来源。
/// @param legacy 旧搜索配置；settings 为插件设置；declared 为包的静态能力声明
/// @returns 仅保存在内存中的设置与包含自定义搜索地址的能力声明
pub(super) fn resolve(
    legacy: &WebSearchConfig,
    settings: &Value,
    declared: &Capabilities,
) -> Result<RuntimeOverrides> {
    let mut merged = serde_json::to_value(legacy)?;
    let object = merged
        .as_object_mut()
        .context("search settings must be an object")?;
    object.remove("enabled");
    for (name, value) in settings
        .as_object()
        .context("plugin settings must be an object")?
    {
        object.insert(name.clone(), value.clone());
    }
    // 1. 【搜索兼容】【凭据读取】保留首个配置密钥、显式环境引用和旧环境变量回退顺序
    for (field, fallback) in [
        ("tinyfish_api_keys", "TINYFISH_API_KEY"),
        ("tavily_api_keys", "TAVILY_API_KEY"),
        ("firecrawl_api_keys", "FIRECRAWL_API_KEY"),
        ("anysearch_api_keys", "ANYSEARCH_API_KEY"),
    ] {
        let values = object
            .get(field)
            .and_then(Value::as_array)
            .with_context(|| format!("web-search.{field} must be an array"))?;
        let key = first_key(values, fallback, field)?;
        object.insert(field.into(), json!(key.into_iter().collect::<Vec<_>>()));
    }
    // 2. 【搜索兼容】【地址声明】只从用户配置的已知地址字段补充来源，不开放任意网络
    let mut capabilities = declared.clone();
    for (field, read_only_post) in [
        ("tinyfish_base_url", false),
        ("tavily_base_url", true),
        ("firecrawl_base_url", true),
        ("anysearch_base_url", true),
        ("searxng_base_url", false),
    ] {
        let value = object
            .get(field)
            .and_then(Value::as_str)
            .with_context(|| format!("web-search.{field} must be a string"))?;
        let trimmed = value.trim();
        let endpoint = if trimmed.is_empty()
            || trimmed.starts_with("https://")
            || trimmed.starts_with("http://")
        {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        };
        // 【搜索兼容】【地址回退】无效地址由请求入口拒绝，不阻止其他供应商回退
        if let Ok(mut url) = Url::parse(&endpoint) {
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
            {
                capabilities.http.insert(url.origin().ascii_serialization());
                if read_only_post {
                    url.set_query(None);
                    url.set_fragment(None);
                    capabilities.http_read_only_post.insert(url.to_string());
                }
            }
        }
        object.insert(field.into(), Value::String(endpoint));
    }
    capabilities.validate()?;
    Ok(RuntimeOverrides {
        settings: merged,
        capabilities,
    })
}

/// 【搜索兼容】【凭据选择】复用旧版首个非空密钥规则，不向插件开放任意环境变量读取。
/// @param values 用户配置的密钥或 $env 引用；fallback 为旧版环境变量名；field 为错误定位字段
/// @returns 首个有效密钥或 None，错误不包含密钥内容
fn first_key(values: &[Value], fallback: &str, field: &str) -> Result<Option<String>> {
    let mut first = None;
    for value in values {
        let Some(value) = value.as_str() else {
            bail!("web-search.{field} must contain only strings");
        };
        if first.is_some() {
            continue;
        }
        let value = value.trim();
        let resolved = if let Some(name) = value.strip_prefix("$env:") {
            std::env::var(name.trim()).ok()
        } else {
            Some(value.to_string())
        };
        first = resolved
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }
    Ok(first.or_else(|| {
        std::env::var(fallback)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }))
}

#[cfg(test)]
mod tests {
    use super::first_key;
    use serde_json::json;

    /// 【搜索测试】【凭据优先级】保留配置值、有效环境引用和最终环境回退的先后顺序。
    #[test]
    fn credentials_preserve_config_reference_and_environment_fallback_order() {
        const FALLBACK: &str = "SAI_SEARCH_CREDENTIAL_FALLBACK_TEST";
        const REFERENCE: &str = "SAI_SEARCH_CREDENTIAL_REFERENCE_TEST";
        const MISSING: &str = "SAI_SEARCH_CREDENTIAL_MISSING_TEST";
        std::env::set_var(FALLBACK, " fallback-key ");
        std::env::set_var(REFERENCE, " referenced-key ");
        std::env::remove_var(MISSING);
        for (values, expected) in [
            (
                json!([" configured-key ", format!("$env:{REFERENCE}")]),
                "configured-key",
            ),
            (
                json!([
                    "",
                    format!("$env: {MISSING} "),
                    format!("$env: {REFERENCE} ")
                ]),
                "referenced-key",
            ),
            (json!(["", format!("$env:{MISSING}")]), "fallback-key"),
            (json!([]), "fallback-key"),
        ] {
            assert_eq!(
                first_key(values.as_array().unwrap(), FALLBACK, "api_keys")
                    .unwrap()
                    .as_deref(),
                Some(expected)
            );
        }
        std::env::remove_var(FALLBACK);
        std::env::remove_var(REFERENCE);
    }
}

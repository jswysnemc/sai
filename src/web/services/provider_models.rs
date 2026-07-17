use super::config_service::SECRET_SENTINEL;
use crate::config::{AppConfig, ProviderConfig};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// 使用当前配置补齐脱敏凭据。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `provider`: 浏览器提交的供应商配置
///
/// 返回:
/// - 可用于模型请求的供应商配置
pub(crate) fn restore_provider_secret(
    paths: &SaiPaths,
    mut provider: ProviderConfig,
) -> Result<ProviderConfig> {
    if provider.api_key.as_deref() != Some(SECRET_SENTINEL) {
        return Ok(provider);
    }
    let current = AppConfig::load_or_default(paths)?;
    provider.api_key = current
        .providers
        .into_iter()
        .find(|item| item.id == provider.id)
        .and_then(|item| item.api_key);
    Ok(provider)
}

/// 获取供应商公开的模型列表。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `provider`: 完整供应商配置
///
/// 返回:
/// - 排序并去重后的模型标识列表
pub(crate) fn fetch_models(paths: &SaiPaths, provider: &ProviderConfig) -> Result<Vec<String>> {
    let api_key = resolve_api_key(paths, provider);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(provider.timeout_seconds.max(1)))
        .build()?;
    let mut last_error = None;
    for url in model_urls(&provider.base_url) {
        let mut request = client
            .get(&url)
            .header("Accept", "application/json")
            .header("User-Agent", "sai-web");
        if !api_key.is_empty() {
            request = request.bearer_auth(&api_key);
        }
        match request.send() {
            Ok(response) => {
                let status = response.status();
                let body = response.text()?;
                if status.is_success() {
                    let parsed: ModelsResponse = serde_json::from_str(&body)?;
                    let mut models = parsed
                        .data
                        .into_iter()
                        .map(|model| model.id)
                        .filter(|id| !id.trim().is_empty())
                        .collect::<Vec<_>>();
                    models.sort();
                    models.dedup();
                    return Ok(models);
                }
                last_error = Some(format!("{status}: {body}"));
                if status.as_u16() != 404 {
                    break;
                }
            }
            Err(error) => {
                last_error = Some(error.to_string());
                break;
            }
        }
    }
    bail!(last_error.unwrap_or_else(|| "模型接口未返回结果".to_string()))
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CatalogMetadata {
    pub(crate) provider: String,
    pub(crate) context_chars: Option<u64>,
    pub(crate) tags: Vec<String>,
}

/// 从 models.dev 目录补充模型元数据。
pub(crate) fn fetch_catalog_metadata(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    else {
        return Vec::new();
    };
    let Ok(response) = client
        .get("https://models.dev/api.json")
        .header("User-Agent", "sai-web")
        .send()
    else {
        return Vec::new();
    };
    let Ok(catalog) = response.json::<Value>() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| {
            find_catalog_model(&catalog, model).map(|metadata| (model.clone(), metadata))
        })
        .collect()
}

fn find_catalog_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    for (provider_id, provider) in catalog.as_object()? {
        let models = provider.get("models")?.as_object()?;
        if let Some(value) = models.get(model).or_else(|| {
            models
                .values()
                .find(|value| value.get("id").and_then(Value::as_str) == Some(model))
        }) {
            let context = value
                .get("limit")
                .and_then(|limit| limit.get("context"))
                .and_then(Value::as_u64);
            return Some(CatalogMetadata {
                provider: provider_id.clone(),
                context_chars: context,
                tags: catalog_model_tags(value),
            });
        }
    }
    None
}

/// 从 models.dev 模型条目推导 Sai 能力标签。
fn catalog_model_tags(value: &Value) -> Vec<String> {
    let mut tags = Vec::new();
    if value.get("tool_call").and_then(Value::as_bool) == Some(true) {
        tags.push("tool".to_string());
    }
    if value.get("reasoning").and_then(Value::as_bool) == Some(true) {
        tags.push("thinking".to_string());
    }
    let supports_image = value
        .get("modalities")
        .and_then(|modalities| modalities.get("input"))
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("image")));
    if supports_image {
        tags.push("vision".to_string());
    }
    tags
}

/// 解析供应商 API Key，缺失时允许无认证模型接口。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `provider`: 供应商配置
///
/// 返回:
/// - API Key 或空字符串
fn resolve_api_key(paths: &SaiPaths, provider: &ProviderConfig) -> String {
    provider.resolved_api_key(paths).unwrap_or_default()
}

/// 生成兼容常见 OpenAI 接口部署方式的模型地址候选。
///
/// 参数:
/// - `base_url`: 供应商基础地址
///
/// 返回:
/// - 按优先级排列的模型接口地址
fn model_urls(base_url: &str) -> Vec<String> {
    let mut base = base_url.trim().trim_end_matches('/').to_string();
    if base.ends_with("/chat/completions") {
        base.truncate(base.len() - "/chat/completions".len());
    }
    if base.ends_with("/v1") || base.ends_with("/openai") {
        return vec![format!("{base}/models")];
    }
    vec![format!("{base}/models"), format!("{base}/v1/models")]
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_openai_and_unversioned_model_urls() {
        assert_eq!(
            model_urls("https://api.example.test/v1"),
            ["https://api.example.test/v1/models"]
        );
        assert_eq!(
            model_urls("https://api.example.test"),
            [
                "https://api.example.test/models",
                "https://api.example.test/v1/models"
            ]
        );
    }
}

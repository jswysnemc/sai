use super::config_service::SECRET_SENTINEL;
use crate::config::{AppConfig, ProviderConfig};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
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

/// 供应商模型接口返回结果。
pub(crate) struct FetchModelsResult {
    pub(crate) models: Vec<String>,
    pub(crate) metadata: BTreeMap<String, CatalogMetadata>,
}

/// 获取供应商公开的模型列表及模型元数据。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `provider`: 完整供应商配置
///
/// 返回:
/// - 排序并去重后的模型标识和元数据
pub(crate) fn fetch_models(
    paths: &SaiPaths,
    provider: &ProviderConfig,
) -> Result<FetchModelsResult> {
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
                    return parse_models_response(&body, &provider.id);
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
    bail!(last_error.unwrap_or_else(|| "model endpoint returned no result".to_string()))
}

/// 解析供应商模型响应，并保留常见的上下文、输出限制和能力字段。
///
/// 参数:
/// - `body`: `/models` 接口响应正文
/// - `provider_id`: 当前供应商标识
///
/// 返回:
/// - 去重后的模型和模型元数据
fn parse_models_response(body: &str, provider_id: &str) -> Result<FetchModelsResult> {
    let parsed: ModelsResponse = serde_json::from_str(body)?;
    let mut models = Vec::new();
    let mut metadata = BTreeMap::new();
    for model in parsed.data {
        let id = model.id.trim().to_string();
        if id.is_empty() || models.iter().any(|item| item == &id) {
            continue;
        }
        let context_chars = model.context_length.or(model.context_window);
        let max_output_tokens = model.max_output_tokens.or(model.max_completion_tokens);
        let tags = model.tags();
        if context_chars.is_some() || max_output_tokens.is_some() || !tags.is_empty() {
            metadata.insert(
                id.clone(),
                CatalogMetadata {
                    provider: provider_id.to_string(),
                    context_chars,
                    max_output_tokens,
                    tags,
                },
            );
        }
        models.push(id);
    }
    models.sort();
    Ok(FetchModelsResult { models, metadata })
}

/// 使用 models.dev 目录补全供应商未返回的模型元数据。
///
/// 参数:
/// - `result`: 供应商模型响应结果
pub(crate) fn enrich_catalog_metadata(result: &mut FetchModelsResult) {
    let catalog = fetch_catalog_metadata(&result.models);
    merge_catalog_metadata(&mut result.metadata, catalog);
}

/// 合并模型目录元数据，供应商返回值具有更高优先级。
///
/// 参数:
/// - `metadata`: 当前模型元数据
/// - `catalog`: models.dev 目录元数据
fn merge_catalog_metadata(
    metadata: &mut BTreeMap<String, CatalogMetadata>,
    catalog: Vec<(String, CatalogMetadata)>,
) {
    for (model, catalog_metadata) in catalog {
        let entry = metadata.entry(model).or_insert_with(|| CatalogMetadata {
            provider: catalog_metadata.provider.clone(),
            context_chars: None,
            max_output_tokens: None,
            tags: Vec::new(),
        });
        if entry.context_chars.is_none() {
            entry.context_chars = catalog_metadata.context_chars;
        }
        if entry.max_output_tokens.is_none() {
            entry.max_output_tokens = catalog_metadata.max_output_tokens;
        }
        for tag in catalog_metadata.tags {
            if !entry.tags.iter().any(|current| current == &tag) {
                entry.tags.push(tag);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct CatalogMetadata {
    pub(crate) provider: String,
    pub(crate) context_chars: Option<u64>,
    pub(crate) max_output_tokens: Option<u64>,
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
            let max_output_tokens = value
                .get("limit")
                .and_then(|limit| limit.get("output"))
                .and_then(Value::as_u64);
            return Some(CatalogMetadata {
                provider: provider_id.clone(),
                context_chars: context,
                max_output_tokens,
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
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    context_window: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<u64>,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
    #[serde(default)]
    capabilities: Vec<String>,
}

impl ModelInfo {
    /// 将供应商能力字段转换为 Sai 模型标签。
    ///
    /// 返回:
    /// - 去重并排序后的标签
    fn tags(&self) -> Vec<String> {
        let mut tags = self
            .capabilities
            .iter()
            .filter_map(|capability| match capability.as_str() {
                "tools" | "tool_calling" => Some("tool"),
                "reasoning" | "thinking" => Some("thinking"),
                "vision" | "image" => Some("vision"),
                "web_search" => Some("web_search"),
                _ => None,
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        tags.sort();
        tags.dedup();
        tags
    }
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

    #[test]
    fn parses_provider_model_limits_and_capabilities() {
        let result = parse_models_response(
            r#"{"data":[{"id":"model-b","context_window":128000,"max_completion_tokens":32768,"capabilities":["reasoning","tools"]},{"id":"model-a","max_output_tokens":8192}]}"#,
            "provider-a",
        )
        .unwrap();

        assert_eq!(result.models, ["model-a", "model-b"]);
        let metadata = result.metadata.get("model-b").unwrap();
        assert_eq!(metadata.provider, "provider-a");
        assert_eq!(metadata.context_chars, Some(128_000));
        assert_eq!(metadata.max_output_tokens, Some(32_768));
        assert_eq!(metadata.tags, ["thinking", "tool"]);
    }

    #[test]
    fn catalog_metadata_only_fills_missing_provider_values() {
        let mut metadata = BTreeMap::from([(
            "model-a".to_string(),
            CatalogMetadata {
                provider: "provider-a".to_string(),
                context_chars: Some(128_000),
                max_output_tokens: None,
                tags: vec!["tool".to_string()],
            },
        )]);

        merge_catalog_metadata(
            &mut metadata,
            vec![(
                "model-a".to_string(),
                CatalogMetadata {
                    provider: "catalog-provider".to_string(),
                    context_chars: Some(64_000),
                    max_output_tokens: Some(16_384),
                    tags: vec!["thinking".to_string(), "tool".to_string()],
                },
            )],
        );

        let merged = metadata.get("model-a").unwrap();
        assert_eq!(merged.provider, "provider-a");
        assert_eq!(merged.context_chars, Some(128_000));
        assert_eq!(merged.max_output_tokens, Some(16_384));
        assert_eq!(merged.tags, ["tool", "thinking"]);
    }
}

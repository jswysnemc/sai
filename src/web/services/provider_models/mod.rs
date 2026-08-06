//! 供应商模型列表获取与公开目录元数据补全。

mod catalog;
mod match_score;
mod response;

use super::config_service::SECRET_SENTINEL;
use crate::config::{AppConfig, ProviderConfig};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;

pub(crate) use catalog::fetch_catalog_metadata;
use catalog::{fetch_litellm_catalog, fetch_models_dev_catalog, fetch_openrouter_catalog};
use response::parse_models_response;

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
    let current = AppConfig::load_or_default(paths)?;
    let current_provider = current
        .providers
        .into_iter()
        .find(|item| item.id == provider.id);
    // 1. 单密钥哨兵回填
    if provider.api_key.as_deref() == Some(SECRET_SENTINEL) {
        provider.api_key = current_provider
            .as_ref()
            .and_then(|item| item.api_key.clone());
    }
    // 2. 多密钥哨兵按稳定 id 回填，避免删除或重排后串用密钥
    if provider
        .api_keys
        .iter()
        .any(|key| key.api_key == SECRET_SENTINEL)
    {
        let current_keys = current_provider
            .as_ref()
            .map(|item| &item.api_keys)
            .cloned()
            .unwrap_or_default();
        let current_by_id: std::collections::HashMap<&str, &str> = current_keys
            .iter()
            .map(|key| (key.id.as_str(), key.api_key.as_str()))
            .collect();
        for key in &mut provider.api_keys {
            if key.api_key == SECRET_SENTINEL {
                if let Some(real) = current_by_id.get(key.id.as_str()) {
                    key.api_key = (*real).to_string();
                }
            }
        }
    }
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
        .user_agent(provider.effective_user_agent())
        .build()?;
    let mut last_error = None;
    for url in model_urls(&provider.base_url) {
        let mut request = client.get(&url).header("Accept", "application/json");
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

/// 使用公开目录补全供应商未返回的模型元数据。
///
/// 参数:
/// - `result`: 供应商模型响应结果
pub(crate) fn enrich_catalog_metadata(result: &mut FetchModelsResult) {
    // 1. models.dev 目录
    // 2. OpenRouter 公开模型目录
    // 3. LiteLLM 价格与上下文目录
    let mut catalog = fetch_models_dev_catalog(&result.models);
    catalog.extend(fetch_openrouter_catalog(&result.models));
    catalog.extend(fetch_litellm_catalog(&result.models));
    merge_catalog_metadata(&mut result.metadata, catalog);
}

/// 合并模型目录元数据，供应商返回值具有更高优先级。
///
/// 参数:
/// - `metadata`: 当前模型元数据
/// - `catalog`: 外部目录元数据
pub(super) fn merge_catalog_metadata(
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
        // 外部目录的供应商标识仅在本地为空时补齐，供前端图标匹配
        if entry.provider.trim().is_empty() {
            entry.provider = catalog_metadata.provider;
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
    if base.ends_with("/responses") {
        base.truncate(base.len() - "/responses".len());
    }
    if base.ends_with("/v1") || base.ends_with("/openai") {
        return vec![format!("{base}/models")];
    }
    vec![format!("{base}/models"), format!("{base}/v1/models")]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::services::provider_models::catalog::{
        find_litellm_model, find_models_dev_model, find_openrouter_model,
    };
    use crate::web::services::provider_models::match_score::{
        match_score, model_id_candidates, rank_catalog_match,
    };
    use crate::web::services::provider_models::response::parse_models_response;

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
    fn parses_openrouter_nested_provider_limits() {
        let result = parse_models_response(
            r#"{"data":[{"id":"anthropic/claude-test","context_length":200000,"top_provider":{"max_completion_tokens":64000},"architecture":{"input_modalities":["text","image"]},"supported_parameters":["tools","reasoning"]}]}"#,
            "openrouter",
        )
        .unwrap();
        let metadata = result.metadata.get("anthropic/claude-test").unwrap();
        assert_eq!(metadata.context_chars, Some(200_000));
        assert_eq!(metadata.max_output_tokens, Some(64_000));
        assert!(metadata.tags.contains(&"tool".to_string()));
        assert!(metadata.tags.contains(&"thinking".to_string()));
        assert!(metadata.tags.contains(&"vision".to_string()));
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

    #[test]
    fn model_match_prefers_exact_and_provider_prefixed_ids() {
        let candidates = model_id_candidates("anthropic/claude-sonnet-4");
        assert_eq!(
            match_score(&candidates, "anthropic/claude-sonnet-4"),
            Some(300)
        );
        // bare id 与候选精确相等时也是最高分
        assert_eq!(match_score(&candidates, "claude-sonnet-4"), Some(300));
        assert!(match_score(&candidates, "gpt-4o").is_none());
    }

    /// 验证目录遍历不会在首个不匹配模型处提前返回，并能选出官方供应商。
    #[test]
    fn models_dev_catalog_skips_non_matching_entries() {
        let catalog = serde_json::json!({
            "nano-gpt": {
                "models": {
                    "other-model": {
                        "id": "other-model",
                        "limit": { "context": 1000, "output": 100 },
                        "tool_call": true
                    },
                    "qwen3.8-max-preview": {
                        "id": "qwen3.8-max-preview",
                        "limit": { "context": 5000, "output": 200 },
                        "tool_call": true,
                        "reasoning": true
                    }
                }
            },
            "alibaba": {
                "models": {
                    "qwen3.8-max-preview": {
                        "id": "qwen3.8-max-preview",
                        "limit": { "context": 1_000_000, "output": 131_072 },
                        "tool_call": true,
                        "reasoning": true,
                        "modalities": { "input": ["text", "image"], "output": ["text"] }
                    }
                }
            },
            "zzz-last": {
                "models": {
                    "unrelated": {
                        "id": "unrelated",
                        "limit": { "context": 8000, "output": 1000 }
                    }
                }
            }
        });

        let metadata = find_models_dev_model(&catalog, "qwen3.8-max-preview").unwrap();
        assert_eq!(metadata.provider, "alibaba");
        assert_eq!(metadata.context_chars, Some(1_000_000));
        assert_eq!(metadata.max_output_tokens, Some(131_072));
        assert!(metadata.tags.contains(&"tool".to_string()));
        assert!(metadata.tags.contains(&"thinking".to_string()));
        assert!(metadata.tags.contains(&"vision".to_string()));
    }

    /// 验证 OpenRouter 目录同样不会因前面不匹配条目提前结束。
    #[test]
    fn openrouter_catalog_skips_non_matching_entries() {
        let catalog = serde_json::json!({
            "data": [
                {
                    "id": "openai/gpt-4o",
                    "context_length": 128000,
                    "top_provider": { "max_completion_tokens": 16384 },
                    "supported_parameters": ["tools"]
                },
                {
                    "id": "qwen/qwen3-max",
                    "context_length": 262144,
                    "top_provider": { "max_completion_tokens": 32768 },
                    "supported_parameters": ["tools", "reasoning"],
                    "architecture": { "input_modalities": ["text"] }
                }
            ]
        });

        let metadata = find_openrouter_model(&catalog, "qwen3-max").unwrap();
        assert_eq!(metadata.provider, "qwen");
        assert_eq!(metadata.context_chars, Some(262_144));
        assert_eq!(metadata.max_output_tokens, Some(32_768));
        assert!(metadata.tags.contains(&"tool".to_string()));
        assert!(metadata.tags.contains(&"thinking".to_string()));
    }

    /// 验证 LiteLLM 目录遍历与官方供应商偏好。
    #[test]
    fn litellm_catalog_skips_non_matching_entries() {
        let catalog = serde_json::json!({
            "sample_spec": {},
            "openai/gpt-4o": {
                "max_input_tokens": 128000,
                "max_output_tokens": 16384,
                "litellm_provider": "openai",
                "supports_function_calling": true
            },
            "dashscope/qwen-max": {
                "max_input_tokens": 30720,
                "max_output_tokens": 8192,
                "litellm_provider": "dashscope",
                "supports_function_calling": true,
                "supports_vision": true
            }
        });

        let metadata = find_litellm_model(&catalog, "qwen-max").unwrap();
        assert_eq!(metadata.provider, "dashscope");
        assert_eq!(metadata.context_chars, Some(30_720));
        assert_eq!(metadata.max_output_tokens, Some(8_192));
        assert!(metadata.tags.contains(&"tool".to_string()));
        assert!(metadata.tags.contains(&"vision".to_string()));
    }

    /// 验证综合排序在同分时优先官方供应商。
    #[test]
    fn official_provider_ranks_higher_on_same_match_score() {
        assert!(rank_catalog_match(300, "alibaba") > rank_catalog_match(300, "nano-gpt"));
        assert!(rank_catalog_match(300, "openai") > rank_catalog_match(300, "openrouter"));
    }
}

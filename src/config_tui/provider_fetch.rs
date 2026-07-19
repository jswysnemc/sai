use crate::config::{ModelMetadata, ProviderConfig};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) struct FetchModelsResult {
    pub(super) models: Vec<String>,
    pub(super) metadata: BTreeMap<String, ModelMetadata>,
}

/// 获取 provider 支持的模型列表。
///
/// 参数:
/// - `provider`: provider 配置
///
/// 返回:
/// - 模型 ID 列表
pub(super) fn fetch_models(provider: &ProviderConfig) -> Result<FetchModelsResult> {
    let api_key = provider.api_key.as_deref().unwrap_or_default();
    let mut api_key = if let Some(env_name) = api_key.strip_prefix("$env:") {
        std::env::var(env_name).unwrap_or_default()
    } else {
        api_key.to_string()
    };
    if api_key.is_empty() && provider.is_opencode_zen() {
        api_key = "public".to_string();
    }
    let url = models_url(&provider.base_url);
    let mut request = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(provider.timeout_seconds))
        .build()?
        .get(url)
        .header("Accept", "application/json")
        .header("User-Agent", "sai-config");
    if !api_key.is_empty() {
        request = request.bearer_auth(api_key);
    }
    let response = request.send()?;
    let status = response.status();
    let body = response.text()?;
    if !status.is_success() {
        bail!("{status}: {body}");
    }
    let parsed: ModelsResponse = serde_json::from_str(&body)?;
    let mut models = Vec::new();
    let mut metadata = BTreeMap::new();
    for model in parsed.data.into_iter().filter(|model| !model.id.is_empty()) {
        let context_chars = model.context_length.or(model.context_window);
        let max_output_tokens = model
            .max_output_tokens
            .or(model.max_completion_tokens)
            .and_then(|value| u32::try_from(value).ok());
        let tags = model.tags();
        if context_chars.is_some() || max_output_tokens.is_some() || !tags.is_empty() {
            metadata.insert(
                model.id.clone(),
                ModelMetadata {
                    context_chars,
                    max_output_tokens,
                    tags,
                    ..ModelMetadata::default()
                },
            );
        }
        models.push(model.id);
    }
    for (model, catalog) in crate::web::services::provider_models::fetch_catalog_metadata(&models) {
        let entry = metadata.entry(model).or_default();
        if entry.context_chars.is_none() {
            entry.context_chars = catalog.context_chars.map(|value| value as usize);
        }
        if entry.max_output_tokens.is_none() {
            entry.max_output_tokens = catalog
                .max_output_tokens
                .and_then(|value| value.try_into().ok());
        }
        if entry.tags.is_empty() {
            entry.tags = catalog.tags;
        }
    }
    Ok(FetchModelsResult { models, metadata })
}

/// 生成模型列表 API 地址。
///
/// 参数:
/// - `base_url`: provider Base URL
///
/// 返回:
/// - `/v1/models` 地址
fn models_url(base_url: &str) -> String {
    let mut url = base_url.trim().trim_end_matches('/').to_string();
    if url.ends_with("/chat/completions") {
        url.truncate(url.len() - "/chat/completions".len());
    }
    if url.ends_with("/v1") {
        format!("{url}/models")
    } else {
        format!("{url}/v1/models")
    }
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ModelInfo {
    id: String,
    #[serde(default)]
    context_length: Option<usize>,
    #[serde(default)]
    context_window: Option<usize>,
    #[serde(default)]
    max_output_tokens: Option<u64>,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
    #[serde(default)]
    capabilities: Vec<String>,
}

impl ModelInfo {
    fn tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        for capability in &self.capabilities {
            match capability.as_str() {
                "tools" | "tool_calling" => tags.push("tool".to_string()),
                "reasoning" | "thinking" => tags.push("thinking".to_string()),
                "vision" | "image" => tags.push("vision".to_string()),
                "web_search" => tags.push("web_search".to_string()),
                _ => {}
            }
        }
        tags.sort();
        tags.dedup();
        tags
    }
}

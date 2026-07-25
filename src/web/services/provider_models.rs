use super::config_service::SECRET_SENTINEL;
use crate::config::{AppConfig, ProviderConfig};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;
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
        let context_chars = model.resolved_context();
        let max_output_tokens = model.resolved_max_output();
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

/// 兼容旧调用：从 models.dev 目录补充模型元数据。
pub(crate) fn fetch_catalog_metadata(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    let mut catalog = fetch_models_dev_catalog(models);
    catalog.extend(fetch_openrouter_catalog(models));
    catalog.extend(fetch_litellm_catalog(models));
    catalog
}

/// 从 models.dev 目录补充模型元数据。
fn fetch_models_dev_catalog(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    let Some(catalog) = models_dev_catalog() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| {
            find_models_dev_model(&catalog, model).map(|metadata| (model.clone(), metadata))
        })
        .collect()
}

/// 缓存 models.dev 目录，避免重复下载。
fn models_dev_catalog() -> Option<&'static Value> {
    static CATALOG: OnceLock<Option<Value>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(8))
                .user_agent("sai/0.1 (+https://github.com/jswysnemc/sai)")
                .build()
                .ok()?;
            let response = client
                .get("https://models.dev/api.json")
                .header("Accept", "application/json")
                .send()
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            response.json::<Value>().ok()
        })
        .as_ref()
}

fn find_models_dev_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    let candidates = model_id_candidates(model);
    let mut best: Option<(usize, CatalogMetadata)> = None;
    for (provider_id, provider) in catalog.as_object()? {
        let models = provider.get("models")?.as_object()?;
        for (model_key, value) in models {
            let remote_id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(model_key.as_str());
            let score = match_score(&candidates, remote_id).or_else(|| match_score(&candidates, model_key))?;
            let context = positive_u64(
                value
                    .get("limit")
                    .and_then(|limit| limit.get("context"))
                    .and_then(Value::as_u64),
            );
            let max_output_tokens = positive_u64(
                value
                    .get("limit")
                    .and_then(|limit| limit.get("output"))
                    .and_then(Value::as_u64),
            );
            if context.is_none() && max_output_tokens.is_none() {
                // 无有效上下文字段时仍可提供标签
                if catalog_model_tags(value).is_empty() {
                    continue;
                }
            }
            let candidate = CatalogMetadata {
                provider: provider_id.clone(),
                context_chars: context,
                max_output_tokens,
                tags: catalog_model_tags(value),
            };
            if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
                best = Some((score, candidate));
            }
        }
    }
    best.map(|(_, metadata)| metadata)
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
    // 输入单价较低时标记 low_cost，仅作提示标签
    if let Some(input_cost) = value
        .get("cost")
        .and_then(|cost| cost.get("input"))
        .and_then(Value::as_f64)
    {
        if input_cost > 0.0 && input_cost <= 0.5 {
            tags.push("low_cost".to_string());
        }
    }
    tags
}

/// 从 OpenRouter 公开目录补全元数据。
fn fetch_openrouter_catalog(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    if models.is_empty() {
        return Vec::new();
    }
    let Some(catalog) = openrouter_catalog() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| {
            find_openrouter_model(&catalog, model).map(|metadata| (model.clone(), metadata))
        })
        .collect()
}

fn openrouter_catalog() -> Option<&'static Value> {
    static CATALOG: OnceLock<Option<Value>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(8))
                .user_agent("sai/0.1 (+https://github.com/jswysnemc/sai)")
                .build()
                .ok()?;
            let response = client
                .get("https://openrouter.ai/api/v1/models")
                .header("Accept", "application/json")
                .send()
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            response.json::<Value>().ok()
        })
        .as_ref()
}

fn find_openrouter_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    let candidates = model_id_candidates(model);
    let data = catalog.get("data")?.as_array()?;
    let mut best: Option<(usize, CatalogMetadata)> = None;
    for item in data {
        let remote_id = item.get("id").and_then(Value::as_str)?;
        let score = match_score(&candidates, remote_id)?;
        let context = positive_u64(
            item.get("context_length")
                .and_then(Value::as_u64)
                .or_else(|| {
                    item.get("top_provider")
                        .and_then(|top| top.get("context_length"))
                        .and_then(Value::as_u64)
                }),
        );
        let max_output_tokens = positive_u64(
            item.get("top_provider")
                .and_then(|top| top.get("max_completion_tokens"))
                .and_then(Value::as_u64),
        );
        let provider = remote_id
            .split('/')
            .next()
            .unwrap_or("openrouter")
            .to_string();
        let mut tags = Vec::new();
        if item.get("reasoning").and_then(Value::as_bool) == Some(true)
            || item
                .get("supported_parameters")
                .and_then(Value::as_array)
                .is_some_and(|params| {
                    params.iter().any(|param| {
                        matches!(
                            param.as_str(),
                            Some("reasoning" | "include_reasoning" | "thinking")
                        )
                    })
                })
        {
            tags.push("thinking".to_string());
        }
        if item
            .get("supported_parameters")
            .and_then(Value::as_array)
            .is_some_and(|params| {
                params
                    .iter()
                    .any(|param| matches!(param.as_str(), Some("tools" | "tool_choice")))
            })
        {
            tags.push("tool".to_string());
        }
        if item
            .get("architecture")
            .and_then(|arch| arch.get("input_modalities"))
            .and_then(Value::as_array)
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("image")))
        {
            tags.push("vision".to_string());
        }
        if let Some(prompt) = item
            .get("pricing")
            .and_then(|pricing| pricing.get("prompt"))
            .and_then(Value::as_str)
            .and_then(|value| value.parse::<f64>().ok())
        {
            // OpenRouter 价格单位为每 token 美元；约合每百万 token < $1 视为低成本
            if prompt > 0.0 && prompt < 0.000001 {
                tags.push("low_cost".to_string());
            }
        }
        if context.is_none() && max_output_tokens.is_none() && tags.is_empty() {
            continue;
        }
        let candidate = CatalogMetadata {
            provider,
            context_chars: context,
            max_output_tokens,
            tags,
        };
        if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
            best = Some((score, candidate));
        }
    }
    best.map(|(_, metadata)| metadata)
}

/// 从 LiteLLM 价格与上下文目录补全元数据。
fn fetch_litellm_catalog(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    if models.is_empty() {
        return Vec::new();
    }
    let Some(catalog) = litellm_catalog() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| find_litellm_model(catalog, model).map(|metadata| (model.clone(), metadata)))
        .collect()
}

fn litellm_catalog() -> Option<&'static Value> {
    static CATALOG: OnceLock<Option<Value>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("sai/0.1 (+https://github.com/jswysnemc/sai)")
                .build()
                .ok()?;
            let response = client
                .get("https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json")
                .header("Accept", "application/json")
                .send()
                .ok()?;
            if !response.status().is_success() {
                return None;
            }
            response.json::<Value>().ok()
        })
        .as_ref()
}

fn find_litellm_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    let candidates = model_id_candidates(model);
    let object = catalog.as_object()?;
    let mut best: Option<(usize, CatalogMetadata)> = None;
    for (key, value) in object {
        if key == "sample_spec" {
            continue;
        }
        let score = match_score(&candidates, key)?;
        let context = positive_u64(
            value
                .get("max_input_tokens")
                .and_then(Value::as_u64)
                .or_else(|| value.get("max_tokens").and_then(Value::as_u64)),
        );
        let max_output_tokens = positive_u64(value.get("max_output_tokens").and_then(Value::as_u64));
        let provider = value
            .get("litellm_provider")
            .and_then(Value::as_str)
            .unwrap_or("litellm")
            .to_string();
        let mut tags = Vec::new();
        if value
            .get("supports_function_calling")
            .and_then(Value::as_bool)
            == Some(true)
        {
            tags.push("tool".to_string());
        }
        if value.get("supports_vision").and_then(Value::as_bool) == Some(true) {
            tags.push("vision".to_string());
        }
        if value.get("supports_reasoning").and_then(Value::as_bool) == Some(true) {
            tags.push("thinking".to_string());
        }
        if value.get("supports_web_search").and_then(Value::as_bool) == Some(true) {
            tags.push("web_search".to_string());
        }
        if let Some(input_cost) = value.get("input_cost_per_token").and_then(Value::as_f64) {
            if input_cost > 0.0 && input_cost < 0.000001 {
                tags.push("low_cost".to_string());
            }
        }
        if context.is_none() && max_output_tokens.is_none() && tags.is_empty() {
            continue;
        }
        let candidate = CatalogMetadata {
            provider,
            context_chars: context,
            max_output_tokens,
            tags,
        };
        if best.as_ref().is_none_or(|(best_score, _)| score > *best_score) {
            best = Some((score, candidate));
        }
    }
    best.map(|(_, metadata)| metadata)
}

/// 生成用于跨目录匹配的模型 ID 候选。
fn model_id_candidates(model: &str) -> Vec<String> {
    let trimmed = model.trim().to_ascii_lowercase();
    let mut candidates = vec![trimmed.clone()];
    if let Some((_, bare)) = trimmed.split_once('/') {
        if !bare.is_empty() {
            candidates.push(bare.to_string());
        }
    }
    if let Some((bare, _)) = trimmed.rsplit_once(':') {
        if !bare.is_empty() {
            candidates.push(bare.to_string());
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

/// 计算本地模型 ID 与远程 ID 的匹配分数，越高越优先。
fn match_score(candidates: &[String], remote: &str) -> Option<usize> {
    let remote = remote.trim().to_ascii_lowercase();
    if remote.is_empty() {
        return None;
    }
    let remote_bare = remote
        .rsplit_once('/')
        .map(|(_, bare)| bare)
        .unwrap_or(remote.as_str());
    let mut best = 0usize;
    for candidate in candidates {
        if candidate == &remote {
            best = best.max(300);
            continue;
        }
        if candidate == remote_bare {
            best = best.max(250);
            continue;
        }
        if remote.ends_with(&format!("/{candidate}")) {
            best = best.max(220);
            continue;
        }
        if candidate.ends_with(&format!("/{remote_bare}")) {
            best = best.max(200);
            continue;
        }
        if (remote_bare.starts_with(candidate) || candidate.starts_with(remote_bare))
            && candidate.len() >= 8
            && remote_bare.len() >= 8
        {
            best = best.max(120);
        }
    }
    (best > 0).then_some(best)
}

fn positive_u64(value: Option<u64>) -> Option<u64> {
    value.filter(|item| *item > 0)
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
    max_model_len: Option<u64>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    architecture: Option<ModelArchitecture>,
    #[serde(default)]
    top_provider: Option<TopProviderInfo>,
    #[serde(default)]
    supported_parameters: Vec<String>,
    #[serde(default)]
    reasoning: Option<bool>,
}

#[derive(Deserialize)]
struct ModelArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    modality: Option<String>,
}

#[derive(Deserialize)]
struct TopProviderInfo {
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

impl ModelInfo {
    fn resolved_context(&self) -> Option<u64> {
        positive_u64(
            self.context_length
                .or(self.context_window)
                .or(self.max_model_len)
                .or_else(|| self.top_provider.as_ref().and_then(|top| top.context_length)),
        )
    }

    fn resolved_max_output(&self) -> Option<u64> {
        positive_u64(
            self.max_output_tokens.or(self.max_completion_tokens).or_else(|| {
                self.top_provider
                    .as_ref()
                    .and_then(|top| top.max_completion_tokens)
            }),
        )
    }

    /// 将供应商能力字段转换为 Sai 模型标签。
    ///
    /// 返回:
    /// - 去重并排序后的标签
    fn tags(&self) -> Vec<String> {
        let mut tags = self
            .capabilities
            .iter()
            .filter_map(|capability| match capability.as_str() {
                "tools" | "tool_calling" | "function_calling" => Some("tool"),
                "reasoning" | "thinking" => Some("thinking"),
                "vision" | "image" | "multimodal" => Some("vision"),
                "web_search" => Some("web_search"),
                _ => None,
            })
            .map(str::to_string)
            .collect::<Vec<_>>();
        if self.reasoning == Some(true)
            || self.supported_parameters.iter().any(|param| {
                matches!(
                    param.as_str(),
                    "reasoning" | "include_reasoning" | "thinking"
                )
            })
        {
            tags.push("thinking".to_string());
        }
        if self
            .supported_parameters
            .iter()
            .any(|param| matches!(param.as_str(), "tools" | "tool_choice"))
        {
            tags.push("tool".to_string());
        }
        if let Some(architecture) = &self.architecture {
            if architecture
                .input_modalities
                .iter()
                .any(|item| item == "image")
                || architecture
                    .modality
                    .as_deref()
                    .is_some_and(|modality| modality.contains("image"))
            {
                tags.push("vision".to_string());
            }
        }
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
}

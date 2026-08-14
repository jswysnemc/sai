//! 公开模型目录抓取与匹配。

use super::match_score::{match_score, model_id_candidates, rank_catalog_match};
use super::CatalogMetadata;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

/// 兼容旧调用：从公开目录补充模型元数据。
///
/// 参数:
/// - `models`: 需要补全的模型 ID 列表
///
/// 返回:
/// - 目录匹配到的模型元数据
pub(crate) fn fetch_catalog_metadata(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    let mut catalog = fetch_models_dev_catalog(models);
    catalog.extend(fetch_openrouter_catalog(models));
    catalog.extend(fetch_litellm_catalog(models));
    catalog
}

/// 从 models.dev 目录补充模型元数据。
///
/// 参数:
/// - `models`: 模型 ID 列表
///
/// 返回:
/// - 命中的 models.dev 元数据
pub(super) fn fetch_models_dev_catalog(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    let Some(catalog) = models_dev_catalog() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| {
            find_models_dev_model(catalog, model).map(|metadata| (model.clone(), metadata))
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

/// 在 models.dev 目录中查找模型元数据。
///
/// 参数:
/// - `catalog`: models.dev JSON
/// - `model`: 本地模型 ID
///
/// 返回:
/// - 最佳匹配元数据
pub(super) fn find_models_dev_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    let candidates = model_id_candidates(model);
    let mut best: Option<(usize, CatalogMetadata)> = None;
    // 1. 遍历全部供应商，不可因单个不匹配提前返回
    // 2. 在同分时优先官方模型族供应商标识，便于图标映射
    for (provider_id, provider) in catalog.as_object()? {
        let Some(models) = provider.get("models").and_then(Value::as_object) else {
            continue;
        };
        for (model_key, value) in models {
            let remote_id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(model_key.as_str());
            let Some(score) =
                match_score(&candidates, remote_id).or_else(|| match_score(&candidates, model_key))
            else {
                continue;
            };
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
                thinking_levels: value
                    .get("reasoning_options")
                    .map(crate::config::thinking_levels_from_reasoning_options)
                    .unwrap_or_default(),
            };
            let ranked = rank_catalog_match(score, &candidate.provider);
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| ranked > *best_score)
            {
                best = Some((ranked, candidate));
            }
        }
    }
    best.map(|(_, metadata)| metadata)
}

/// 从 models.dev 模型条目推导 Sai 能力标签。
///
/// 参数:
/// - `value`: models.dev 模型 JSON
///
/// 返回:
/// - 能力标签列表
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
///
/// 参数:
/// - `models`: 模型 ID 列表
///
/// 返回:
/// - 命中的 OpenRouter 元数据
pub(super) fn fetch_openrouter_catalog(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    if models.is_empty() {
        return Vec::new();
    }
    let Some(catalog) = openrouter_catalog() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| {
            find_openrouter_model(catalog, model).map(|metadata| (model.clone(), metadata))
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

/// 在 OpenRouter 目录中查找模型元数据。
///
/// 参数:
/// - `catalog`: OpenRouter JSON
/// - `model`: 本地模型 ID
///
/// 返回:
/// - 最佳匹配元数据
pub(super) fn find_openrouter_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    let candidates = model_id_candidates(model);
    let data = catalog.get("data")?.as_array()?;
    let mut best: Option<(usize, CatalogMetadata)> = None;
    // 遍历全部条目；不匹配项必须 continue，不能用 ? 提前结束
    for item in data {
        let Some(remote_id) = item.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(score) = match_score(&candidates, remote_id) else {
            continue;
        };
        let context = positive_u64(item.get("context_length").and_then(Value::as_u64).or_else(
            || {
                item.get("top_provider")
                    .and_then(|top| top.get("context_length"))
                    .and_then(Value::as_u64)
            },
        ));
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
            // 该目录只公布是否支持推理，不枚举等级，留空表示未知
            thinking_levels: Vec::new(),
        };
        let ranked = rank_catalog_match(score, &candidate.provider);
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| ranked > *best_score)
        {
            best = Some((ranked, candidate));
        }
    }
    best.map(|(_, metadata)| metadata)
}

/// 从 LiteLLM 价格与上下文目录补全元数据。
///
/// 参数:
/// - `models`: 模型 ID 列表
///
/// 返回:
/// - 命中的 LiteLLM 元数据
pub(super) fn fetch_litellm_catalog(models: &[String]) -> Vec<(String, CatalogMetadata)> {
    if models.is_empty() {
        return Vec::new();
    }
    let Some(catalog) = litellm_catalog() else {
        return Vec::new();
    };
    models
        .iter()
        .filter_map(|model| {
            find_litellm_model(catalog, model).map(|metadata| (model.clone(), metadata))
        })
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

/// 在 LiteLLM 目录中查找模型元数据。
///
/// 参数:
/// - `catalog`: LiteLLM JSON
/// - `model`: 本地模型 ID
///
/// 返回:
/// - 最佳匹配元数据
pub(super) fn find_litellm_model(catalog: &Value, model: &str) -> Option<CatalogMetadata> {
    let candidates = model_id_candidates(model);
    let object = catalog.as_object()?;
    let mut best: Option<(usize, CatalogMetadata)> = None;
    // 遍历全部模型键；不匹配项必须 continue，不能用 ? 提前结束
    for (key, value) in object {
        if key == "sample_spec" {
            continue;
        }
        let Some(score) = match_score(&candidates, key) else {
            continue;
        };
        let context = positive_u64(
            value
                .get("max_input_tokens")
                .and_then(Value::as_u64)
                .or_else(|| value.get("max_tokens").and_then(Value::as_u64)),
        );
        let max_output_tokens =
            positive_u64(value.get("max_output_tokens").and_then(Value::as_u64));
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
            // 该目录只公布是否支持推理，不枚举等级，留空表示未知
            thinking_levels: Vec::new(),
        };
        let ranked = rank_catalog_match(score, &candidate.provider);
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| ranked > *best_score)
        {
            best = Some((ranked, candidate));
        }
    }
    best.map(|(_, metadata)| metadata)
}

fn positive_u64(value: Option<u64>) -> Option<u64> {
    value.filter(|item| *item > 0)
}

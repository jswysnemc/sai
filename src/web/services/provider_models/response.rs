//! 供应商 `/models` 响应解析。

use super::{CatalogMetadata, FetchModelsResult};
use anyhow::Result;
use serde::Deserialize;

/// 解析供应商模型响应，并保留常见的上下文、输出限制和能力字段。
///
/// 参数:
/// - `body`: `/models` 接口响应正文
/// - `provider_id`: 当前供应商标识
///
/// 返回:
/// - 去重后的模型和模型元数据
pub(super) fn parse_models_response(body: &str, provider_id: &str) -> Result<FetchModelsResult> {
    let parsed: ModelsResponse = serde_json::from_str(body)?;
    let mut models = Vec::new();
    let mut metadata = std::collections::BTreeMap::new();
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

fn positive_u64(value: Option<u64>) -> Option<u64> {
    value.filter(|item| *item > 0)
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
                .or_else(|| {
                    self.top_provider
                        .as_ref()
                        .and_then(|top| top.context_length)
                }),
        )
    }

    fn resolved_max_output(&self) -> Option<u64> {
        positive_u64(
            self.max_output_tokens
                .or(self.max_completion_tokens)
                .or_else(|| {
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

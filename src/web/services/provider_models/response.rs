//! 供应商 `/models` 响应解析。

use super::reasoning_field::ReasoningField;
use super::{CatalogMetadata, FetchModelsResult};
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;

/// 解析供应商模型响应，并保留常见的上下文、输出限制和能力字段。
///
/// 单个模型条目的字段写法千差万别，因此逐条解析：一条不认识的记录只跳过它自己，
/// 不牵连同一份响应里其余能用的模型。
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
    let total = parsed.data.len();
    let mut skipped = 0usize;
    for entry in parsed.data {
        // 1. 逐条解析，字段类型对不上的记录跳过而不是让整份响应失败
        let Ok(model) = serde_json::from_value::<ModelInfo>(entry) else {
            skipped += 1;
            continue;
        };
        let id = model.id.trim().to_string();
        if id.is_empty() || models.iter().any(|item| item == &id) {
            continue;
        }
        let context_chars = model.resolved_context();
        let max_output_tokens = model.resolved_max_output();
        let tags = model.tags();
        let thinking_levels = model.thinking_levels();
        if context_chars.is_some()
            || max_output_tokens.is_some()
            || !tags.is_empty()
            || !thinking_levels.is_empty()
        {
            metadata.insert(
                id.clone(),
                CatalogMetadata {
                    provider: provider_id.to_string(),
                    context_chars,
                    max_output_tokens,
                    tags,
                    thinking_levels,
                },
            );
        }
        models.push(id);
    }
    // 2. 全部条目都解析不了说明响应结构整体不兼容，此时报错比返回空列表更清楚
    if models.is_empty() && skipped > 0 {
        bail!("model endpoint returned {total} entries but none could be parsed");
    }
    models.sort();
    Ok(FetchModelsResult { models, metadata })
}

fn positive_u64(value: Option<u64>) -> Option<u64> {
    value.filter(|item| *item > 0)
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<Value>,
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
    reasoning: Option<ReasoningField>,
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

    /// 取出该模型支持的思考等级。
    ///
    /// 返回:
    /// - 供应商公布的等级；未公布时为空，表示界面不作限制
    fn thinking_levels(&self) -> Vec<String> {
        self.reasoning
            .as_ref()
            .map(ReasoningField::thinking_levels)
            .unwrap_or_default()
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
        if self
            .reasoning
            .as_ref()
            .is_some_and(ReasoningField::supports_thinking)
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
    use super::parse_models_response;

    /// 验证单条字段对不上时只跳过该条，其余模型仍可用。
    #[test]
    fn skips_unparseable_entries_without_failing_the_response() {
        let result = parse_models_response(
            r#"{"data":[{"id":123},{"id":"good-model","context_window":8000}]}"#,
            "provider-a",
        )
        .unwrap();

        assert_eq!(result.models, ["good-model"]);
        assert_eq!(
            result.metadata.get("good-model").unwrap().context_chars,
            Some(8000)
        );
    }

    /// 验证全部条目都解析不了时返回错误而不是空列表。
    #[test]
    fn fails_when_every_entry_is_unparseable() {
        let error = parse_models_response(r#"{"data":[{"id":1},{"id":2}]}"#, "provider-a")
            .unwrap_err();

        assert!(error.to_string().contains("none could be parsed"));
    }

    /// 验证 OpenRouter 的 reasoning 对象能抽出思考等级，而不是让整份响应失败。
    #[test]
    fn reads_openrouter_reasoning_object_into_thinking_levels() {
        let result = parse_models_response(
            r#"{"data":[{"id":"or-model","reasoning":{"mandatory":true,"supported_efforts":["max","high","low"]}}]}"#,
            "openrouter",
        )
        .unwrap();
        let metadata = result.metadata.get("or-model").unwrap();

        assert_eq!(metadata.thinking_levels, ["low", "high", "max"]);
        assert!(metadata.tags.contains(&"thinking".to_string()));
    }
}

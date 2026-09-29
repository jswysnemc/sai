use crate::config::{ModelMetadata, ProviderConfig};
use anyhow::Result;
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(super) struct FetchModelsResult {
    pub(super) models: Vec<String>,
    pub(super) metadata: BTreeMap<String, ModelMetadata>,
}

/// 【服务商配置】【模型目录】使用共享服务获取目录，保留选中密钥和环境变量语义。
/// 参数: provider 为完整草稿；返回: 模型列表及元数据，不访问外部公共目录
pub(super) fn fetch_models(provider: &ProviderConfig) -> Result<FetchModelsResult> {
    let paths = crate::paths::SaiPaths::new()?;
    let result = crate::web::services::provider_models::fetch_models(&paths, provider)?;
    let metadata = result
        .metadata
        .into_iter()
        .map(|(model, value)| {
            (
                model,
                ModelMetadata {
                    context_chars: value.context_chars.and_then(|value| value.try_into().ok()),
                    max_output_tokens: value
                        .max_output_tokens
                        .and_then(|value| value.try_into().ok()),
                    tags: value.tags,
                    thinking_levels: value.thinking_levels,
                    ..ModelMetadata::default()
                },
            )
        })
        .collect();
    Ok(FetchModelsResult {
        models: result.models,
        metadata,
    })
}

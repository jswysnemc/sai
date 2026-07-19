use super::model::ProviderConfig;
use super::model_metadata::{
    is_valid_model_tag, WEB_SEARCH_TOOL_MODE_ENABLED, WEB_SEARCH_TOOL_MODE_HIDE,
    WEB_SEARCH_TOOL_MODE_RENAME,
};
use anyhow::{bail, Result};

/// 返回官方模型家族的上下文长度 fallback。
///
/// 参数:
/// - `provider`: 当前供应商
/// - `model`: 当前模型 ID
///
/// 返回:
/// - 官方 Anthropic Claude 返回 200K，其他返回空
pub(super) fn default_context_chars_for_provider_model(
    provider: &ProviderConfig,
    model: &str,
) -> Option<usize> {
    let model = model.to_ascii_lowercase();
    (provider.uses_official_anthropic_api() && model.starts_with("claude-")).then_some(200_000)
}

/// 校验 Provider 的模型元数据。
///
/// 参数:
/// - `provider`: Provider 配置
///
/// 返回:
/// - 配置是否合法
pub(super) fn validate_provider_model_metadata(provider: &ProviderConfig) -> Result<()> {
    for (model, context_chars) in &provider.model_context_chars {
        if model.trim().is_empty() {
            bail!(
                "provider {} model_context_chars key cannot be empty",
                provider.id
            );
        }
        if *context_chars == 0 {
            bail!(
                "provider {} model_context_chars for {} must be greater than 0",
                provider.id,
                model
            );
        }
    }

    for (model, metadata) in &provider.model_metadata {
        if model.trim().is_empty() {
            bail!(
                "provider {} model_metadata key cannot be empty",
                provider.id
            );
        }
        if metadata.context_chars == Some(0) {
            bail!(
                "provider {} model_metadata context_chars for {} must be greater than 0",
                provider.id,
                model
            );
        }
        if metadata.max_output_tokens == Some(0) {
            bail!(
                "provider {} model_metadata max_output_tokens for {} must be greater than 0",
                provider.id,
                model
            );
        }
        for tag in &metadata.tags {
            if !is_valid_model_tag(tag) {
                bail!(
                    "provider {} model_metadata tag for {} is invalid: {}",
                    provider.id,
                    model,
                    tag
                );
            }
        }
        if let Some(mode) = metadata.web_search_tool_mode.as_deref() {
            if mode != WEB_SEARCH_TOOL_MODE_ENABLED
                && mode != WEB_SEARCH_TOOL_MODE_HIDE
                && mode != WEB_SEARCH_TOOL_MODE_RENAME
            {
                bail!(
                    "provider {} model_metadata web_search_tool_mode for {} is invalid: {}",
                    provider.id,
                    model,
                    mode
                );
            }
        }
    }
    Ok(())
}

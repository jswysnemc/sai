use crate::config::{parse_context_chars, ProviderConfig, MODEL_TAGS};
use crate::config::{WEB_SEARCH_TOOL_MODE_HIDE, WEB_SEARCH_TOOL_MODE_RENAME};
use crate::i18n::text as t;
use anyhow::Result;

use super::form::{parse_bool_field, Field};

/// 返回模型上下文 token 字段值。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 表单展示的上下文 token 数
pub(super) fn context_chars_field_value(provider: &ProviderConfig, model: &str) -> String {
    provider
        .model_context_chars_for(model)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

/// 返回模型标签勾选字段。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 模型标签勾选字段
pub(super) fn tag_fields(provider: &ProviderConfig, model: &str) -> Vec<Field> {
    MODEL_TAGS
        .iter()
        .map(|tag| {
            Field::boolean(
                tag,
                provider
                    .model_tags_for(model)
                    .iter()
                    .any(|item| item.as_str() == *tag),
            )
        })
        .collect()
}

/// 返回模型工具调用支持字段。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 模型工具调用支持字段
pub(super) fn tools_enabled_field(provider: &ProviderConfig, model: &str) -> Field {
    Field::boolean(
        t("Tool calling support", "工具调用支持"),
        provider.model_tools_enabled_for(model),
    )
}

/// 返回网页搜索工具冲突策略字段。
pub(super) fn web_search_tool_mode_field(provider: &ProviderConfig, model: &str) -> Field {
    Field::new(
        t("Web search tool conflict", "网页搜索工具冲突"),
        provider
            .model_web_search_tool_mode_for(model)
            .unwrap_or(WEB_SEARCH_TOOL_MODE_HIDE)
            .to_string(),
    )
    .choices(&[WEB_SEARCH_TOOL_MODE_HIDE, WEB_SEARCH_TOOL_MODE_RENAME])
}

/// 应用网页搜索工具冲突策略字段。
pub(super) fn apply_web_search_tool_mode_field(
    provider: &mut ProviderConfig,
    model: &str,
    value: &str,
) {
    provider.set_model_web_search_tool_mode(model, Some(value.trim().to_string()));
}

/// 应用模型上下文 token 字段。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
/// - `value`: 表单输入的上下文 token 数
///
/// 返回:
/// - 应用是否成功
pub(super) fn apply_context_chars_field(
    provider: &mut ProviderConfig,
    model: &str,
    value: &str,
) -> Result<()> {
    provider.set_model_context_chars_for(model, parse_context_chars(value)?);
    Ok(())
}

/// 应用模型工具调用支持字段。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
/// - `value`: 工具调用支持字段值
///
/// 返回:
/// - 应用是否成功
pub(super) fn apply_tools_enabled_field(
    provider: &mut ProviderConfig,
    model: &str,
    value: &str,
) -> Result<()> {
    provider.set_model_tools_enabled_for(model, parse_bool_field(value)?);
    Ok(())
}

/// 应用模型标签勾选字段。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
/// - `fields`: 模型标签勾选字段
///
/// 返回:
/// - 应用是否成功
pub(super) fn apply_tag_fields(
    provider: &mut ProviderConfig,
    model: &str,
    fields: &[Field],
) -> Result<()> {
    let mut tags = Vec::new();
    for field in fields {
        if parse_bool_field(&field.value)? {
            tags.push(field.label.to_string());
        }
    }
    provider.set_model_tags_for(model, tags);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelMetadata;

    fn provider_with_model(model: &str) -> ProviderConfig {
        let mut provider = ProviderConfig::new_openai_compatible();
        provider.models.push(model.to_string());
        provider.default_model = model.to_string();
        provider
    }

    #[test]
    fn applies_unit_context_to_model_metadata() {
        let mut provider = provider_with_model("test-model");

        apply_context_chars_field(&mut provider, "test-model", "128k").unwrap();

        assert_eq!(
            provider
                .model_metadata
                .get("test-model")
                .and_then(|metadata| metadata.context_chars),
            Some(128_000)
        );
    }

    #[test]
    fn applies_checked_tags() {
        let mut provider = provider_with_model("test-model");
        let mut fields = tag_fields(&provider, "test-model");
        fields[0].value = "true".to_string();
        fields[2].value = "true".to_string();

        apply_tag_fields(&mut provider, "test-model", &fields).unwrap();

        assert_eq!(
            provider.model_tags_for("test-model"),
            &["tool".to_string(), "vision".to_string()]
        );
    }

    #[test]
    fn applies_disabled_tool_support() {
        let mut provider = provider_with_model("test-model");

        apply_tools_enabled_field(&mut provider, "test-model", "false").unwrap();

        assert!(!provider.model_tools_enabled_for("test-model"));
        assert_eq!(
            provider
                .model_metadata
                .get("test-model")
                .and_then(|metadata| metadata.tools_enabled),
            Some(false)
        );
    }

    #[test]
    fn enabling_tool_support_removes_default_metadata() {
        let mut provider = provider_with_model("test-model");
        apply_tools_enabled_field(&mut provider, "test-model", "false").unwrap();
        apply_tools_enabled_field(&mut provider, "test-model", "true").unwrap();

        assert!(provider.model_tools_enabled_for("test-model"));
        assert!(!provider.model_metadata.contains_key("test-model"));
    }

    #[test]
    fn reads_legacy_context_when_metadata_is_empty() {
        let mut provider = provider_with_model("test-model");
        provider
            .model_context_chars
            .insert("test-model".to_string(), 42_000);
        provider
            .model_metadata
            .insert("other-model".to_string(), ModelMetadata::default());

        assert_eq!(context_chars_field_value(&provider, "test-model"), "42000");
    }
}

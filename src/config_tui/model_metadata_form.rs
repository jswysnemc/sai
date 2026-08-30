use crate::config::{parse_context_chars, ProviderConfig, MODEL_TAGS};
use crate::config::{
    DEEPSEEK_ANCHOR_MODE_OFF, DEEPSEEK_ANCHOR_MODE_STANDARD, WEB_SEARCH_TOOL_MODE_ENABLED,
    WEB_SEARCH_TOOL_MODE_HIDE, WEB_SEARCH_TOOL_MODE_RENAME,
};
use crate::i18n::text as t;
use anyhow::{bail, Result};

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

/// 返回模型最大输出 token 字段值。
pub(super) fn max_output_tokens_field_value(provider: &ProviderConfig, model: &str) -> String {
    provider
        .model_max_output_tokens_for(model)
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
        provider.model_web_search_tool_mode_for(model).to_string(),
    )
    .choices(&[
        WEB_SEARCH_TOOL_MODE_ENABLED,
        WEB_SEARCH_TOOL_MODE_HIDE,
        WEB_SEARCH_TOOL_MODE_RENAME,
    ])
}

/// 返回 DeepSeek Anchored Standard 模式字段。
pub(super) fn deepseek_anchor_mode_field(provider: &ProviderConfig, model: &str) -> Field {
    Field::new(
        t("DeepSeek trajectory anchor", "DeepSeek 轨迹锚定"),
        provider.model_deepseek_anchor_mode_for(model).to_string(),
    )
    .choices(&[DEEPSEEK_ANCHOR_MODE_OFF, DEEPSEEK_ANCHOR_MODE_STANDARD])
}

/// 应用 DeepSeek Anchored Standard 模式字段。
pub(super) fn apply_deepseek_anchor_mode_field(
    provider: &mut ProviderConfig,
    model: &str,
    value: &str,
) {
    provider.set_model_deepseek_anchor_mode_for(model, value);
}

/// 应用网页搜索工具冲突策略字段。
pub(super) fn apply_web_search_tool_mode_field(
    provider: &mut ProviderConfig,
    model: &str,
    value: &str,
) {
    let value = value.trim();
    provider.set_model_web_search_tool_mode(
        model,
        (value != WEB_SEARCH_TOOL_MODE_ENABLED).then(|| value.to_string()),
    );
}

/// 解析模型最大输出 token 字段（只校验，不修改配置）。
///
/// 参数:
/// - `value`: 表单输入的最大输出 token 数
///
/// 返回:
/// - 解析后的值，空输入表示不限制
pub(super) fn parse_max_output_tokens(value: &str) -> Result<Option<u32>> {
    let value = parse_context_chars(value)?;
    let value = value
        .map(|value| {
            u32::try_from(value).map_err(|_| anyhow::anyhow!("max output tokens is too large"))
        })
        .transpose()?;
    if value == Some(0) {
        bail!("max output tokens must be greater than 0");
    }
    Ok(value)
}

/// 解析模型上下文字段（只校验，不修改配置）。
///
/// 参数:
/// - `value`: 表单输入的上下文 token 数
///
/// 返回:
/// - 解析后的值，空输入表示不限制
pub(super) fn parse_context_chars_field(value: &str) -> Result<Option<usize>> {
    parse_context_chars(value)
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

/// 返回模型支持的思考等级勾选字段。
///
/// 一个都不勾表示不限制：模型目录未覆盖或数据有误时，
/// 清空即恢复展示全部等级。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 思考等级勾选字段
pub(super) fn thinking_level_fields(provider: &ProviderConfig, model: &str) -> Vec<Field> {
    crate::config::THINKING_LEVELS
        .iter()
        .map(|level| {
            Field::boolean(
                level,
                provider
                    .model_thinking_levels_for(model)
                    .iter()
                    .any(|item| item.as_str() == *level),
            )
        })
        .collect()
}

/// 应用模型思考等级勾选字段。
///
/// 参数:
/// - `provider`: Provider 配置
/// - `model`: 模型 ID
/// - `fields`: 思考等级勾选字段
///
/// 返回:
/// - 应用是否成功
pub(super) fn apply_thinking_level_fields(
    provider: &mut ProviderConfig,
    model: &str,
    fields: &[Field],
) -> Result<()> {
    let mut levels = Vec::new();
    for field in fields {
        if parse_bool_field(&field.value)? {
            levels.push(field.label.to_string());
        }
    }
    provider.set_model_thinking_levels_for(model, levels);
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

        provider
            .set_model_context_chars_for("test-model", parse_context_chars_field("128k").unwrap());

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
    fn applies_max_output_tokens() {
        let mut provider = provider_with_model("test-model");

        // apply_max_output_tokens_field 已拆分为 parse_max_output_tokens + setter：
        // 表单需要先全量校验再落地，避免中途报错把 provider 改坏一半
        provider
            .set_model_max_output_tokens_for("test-model", parse_max_output_tokens("32k").unwrap());

        assert_eq!(
            provider.model_max_output_tokens_for("test-model"),
            Some(32_000)
        );
    }

    #[test]
    fn web_search_defaults_to_enabled_without_tag() {
        let mut provider = provider_with_model("test-model");
        assert_eq!(
            provider.model_web_search_tool_mode_for("test-model"),
            WEB_SEARCH_TOOL_MODE_ENABLED
        );

        apply_web_search_tool_mode_field(&mut provider, "test-model", WEB_SEARCH_TOOL_MODE_HIDE);
        assert_eq!(
            provider.model_web_search_tool_mode_for("test-model"),
            WEB_SEARCH_TOOL_MODE_HIDE
        );
    }

    #[test]
    fn applies_disabled_tool_support() {
        let mut provider = provider_with_model("test-model");

        provider.set_model_tools_enabled_for("test-model", parse_bool_field("false").unwrap());

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
        provider.set_model_tools_enabled_for("test-model", parse_bool_field("false").unwrap());
        provider.set_model_tools_enabled_for("test-model", parse_bool_field("true").unwrap());

        assert!(provider.model_tools_enabled_for("test-model"));
        assert!(!provider.model_metadata.contains_key("test-model"));
    }

    #[test]
    fn applies_deepseek_anchor_mode() {
        let mut provider = provider_with_model("deepseek-v4");
        apply_deepseek_anchor_mode_field(
            &mut provider,
            "deepseek-v4",
            DEEPSEEK_ANCHOR_MODE_STANDARD,
        );

        assert_eq!(
            provider.model_deepseek_anchor_mode_for("deepseek-v4"),
            DEEPSEEK_ANCHOR_MODE_STANDARD
        );
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

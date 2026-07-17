use crate::config::ProviderConfig;
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::Value;
use std::io;

use super::form::{parse_bool_field, run_form, Field};
use super::model_metadata_form::{
    apply_context_chars_field, apply_tag_fields, apply_tools_enabled_field,
    apply_web_search_tool_mode_field, context_chars_field_value, tag_fields, tools_enabled_field,
    web_search_tool_mode_field,
};

/// 编辑 provider 配置表单。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `provider`: 原 provider 配置
///
/// 返回:
/// - 修改后的 provider 配置，取消时返回空
pub(super) fn edit_provider_form(
    stdout: &mut io::Stdout,
    provider: ProviderConfig,
) -> Result<Option<ProviderConfig>> {
    let current_context_chars = context_chars_field_value(&provider, &provider.default_model);
    let mut fields = vec![
        Field::new(t("Config ID", "配置 ID"), provider.id.clone()),
        Field::new(t("Display name", "显示名称"), provider.display_name.clone()),
        Field::new("Base URL", provider.base_url.clone()),
        Field::new(t("Protocol", "协议"), provider.protocol.clone()).choices(&[
            "auto",
            "openai-chat",
            "openai-responses",
            "anthropic",
        ]),
        Field::new(
            "API Key or $env:NAME",
            provider.api_key.clone().unwrap_or_default(),
        )
        .secret(),
        Field::new(
            t("Current model", "当前模型"),
            provider.default_model.clone(),
        ),
        Field::new(
            t("Model context tokens", "模型上下文 token 数"),
            current_context_chars,
        ),
    ];
    let tag_start = fields.len();
    fields.extend(tag_fields(&provider, &provider.default_model));
    let tag_end = fields.len();
    let web_search_mode_index = provider
        .model_tags_for(&provider.default_model)
        .iter()
        .any(|tag| tag == "web_search")
        .then(|| {
            fields.push(web_search_tool_mode_field(
                &provider,
                &provider.default_model,
            ));
            fields.len() - 1
        });
    fields.extend([
        Field::new(
            t("Timeout seconds", "超时秒数"),
            provider.timeout_seconds.to_string(),
        ),
        Field::new("Temperature", provider.temperature.to_string()),
        Field::new(
            t("Thinking level", "思考等级"),
            provider.thinking_level.clone(),
        )
        .choices(&["auto", "none", "low", "medium", "high", "xhigh", "max"]),
        Field::new(
            t("Thinking format", "思考格式"),
            provider.thinking_format.clone(),
        )
        .choices(&[
            "auto",
            "string",
            "object",
            "deepseek-thinking",
            "openai-chat-reasoning-effort",
            "reasoning",
            "anthropic-thinking",
            "disabled",
        ]),
        Field::textarea(
            t("Custom Body JSON", "自定义 Body JSON"),
            provider.extra_body.clone(),
        ),
    ]);
    if !run_form(stdout, t(" EDIT PROVIDER ", " 编辑供应商 "), &mut fields)? {
        return Ok(None);
    }
    let default_model = fields[5].value.trim().to_string();
    let mut models = provider.models.clone();
    if !default_model.trim().is_empty() && !models.iter().any(|item| item == &default_model) {
        models.push(default_model.clone());
    }
    let behavior_start = tag_end + usize::from(web_search_mode_index.is_some());
    let extra_body = normalize_extra_body(&fields[behavior_start + 4].value)?;
    let mut updated = ProviderConfig {
        id: fields[0].value.trim().to_string(),
        display_name: fields[1].value.trim().to_string(),
        base_url: normalize_base_url(&fields[2].value),
        protocol: fields[3].value.trim().to_string(),
        api_key: Some(fields[4].value.trim().to_string()).filter(|value| !value.is_empty()),
        models,
        model_context_chars: provider.model_context_chars.clone(),
        model_metadata: provider.model_metadata.clone(),
        default_model,
        timeout_seconds: fields[behavior_start].value.trim().parse().unwrap_or(60),
        temperature: fields[behavior_start + 1]
            .value
            .trim()
            .parse()
            .unwrap_or(0.7),
        anthropic_max_tokens: provider.anthropic_max_tokens,
        thinking_level: fields[behavior_start + 2].value.trim().to_string(),
        thinking_format: fields[behavior_start + 3].value.trim().to_string(),
        extra_body,
    };
    let default_model = updated.default_model.clone();
    apply_context_chars_field(&mut updated, &default_model, &fields[6].value)?;
    apply_tag_fields(&mut updated, &default_model, &fields[tag_start..tag_end])?;
    if let Some(index) = web_search_mode_index {
        apply_web_search_tool_mode_field(&mut updated, &default_model, &fields[index].value);
    }
    Ok(Some(updated))
}

/// 规范化并校验自定义 Body JSON。
///
/// 参数:
/// - `value`: 表单中输入的 JSON 文本
///
/// 返回:
/// - 为空时返回空字符串，否则返回格式化后的 JSON 对象字符串
fn normalize_extra_body(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let parsed = serde_json::from_str::<Value>(value)?;
    if !parsed.is_object() {
        bail!(
            "{}",
            t(
                "Custom Body JSON must be a JSON object",
                "自定义 Body JSON 必须是 JSON 对象"
            )
        );
    }
    Ok(serde_json::to_string_pretty(&parsed)?)
}

/// 编辑模型配置表单。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `provider`: 当前 provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 是否保存
pub(super) fn edit_model_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<bool> {
    let active = provider.models.iter().any(|item| item == model);
    let current = provider.default_model == model;
    let context_chars = context_chars_field_value(provider, model);
    let mut fields = vec![
        Field::boolean(t("Activate model", "激活模型"), active),
        Field::boolean(t("Set as current model", "设为当前模型"), current),
        tools_enabled_field(provider, model),
        Field::new(
            t("Model context tokens", "模型上下文 token 数"),
            context_chars,
        ),
    ];
    let tag_start = fields.len();
    fields.extend(tag_fields(provider, model));
    let tag_end = fields.len();
    let web_search_mode_index = provider
        .model_tags_for(model)
        .iter()
        .any(|tag| tag == "web_search")
        .then(|| {
            fields.push(web_search_tool_mode_field(provider, model));
            fields.len() - 1
        });
    if !run_form(stdout, t(" EDIT MODEL ", " 编辑模型 "), &mut fields)? {
        return Ok(false);
    }
    let active = parse_bool_field(&fields[0].value)?;
    let current = parse_bool_field(&fields[1].value)?;
    if active {
        if !provider.models.iter().any(|item| item == model) {
            provider.models.push(model.to_string());
        }
    } else {
        provider.models.retain(|item| item != model);
    }
    if current || provider.default_model == model && !active {
        provider.default_model = if active {
            model.to_string()
        } else {
            provider.models.first().cloned().unwrap_or_default()
        };
        if !provider.default_model.is_empty()
            && !provider
                .models
                .iter()
                .any(|item| item == &provider.default_model)
        {
            provider.models.push(provider.default_model.clone());
        }
    }
    apply_tools_enabled_field(provider, model, &fields[2].value)?;
    apply_context_chars_field(provider, model, &fields[3].value)?;
    apply_tag_fields(provider, model, &fields[tag_start..tag_end])?;
    if let Some(index) = web_search_mode_index {
        apply_web_search_tool_mode_field(provider, model, &fields[index].value);
    }
    Ok(true)
}

/// 规范化 provider Base URL。
///
/// 参数:
/// - `value`: 表单输入值
///
/// 返回:
/// - 去除末尾斜杠和 chat completions 后缀后的 URL
fn normalize_base_url(value: &str) -> String {
    let mut url = value.trim().trim_end_matches('/').to_string();
    if url.ends_with("/chat/completions") {
        url.truncate(url.len() - "/chat/completions".len());
    }
    url
}

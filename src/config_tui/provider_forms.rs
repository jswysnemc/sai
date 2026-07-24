use crate::config::ProviderConfig;
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::Value;
use std::io;

use super::form::{parse_bool_field, run_form, Field};
use super::input::read_key;
use super::model_metadata_form::{
    apply_context_chars_field, apply_max_output_tokens_field, apply_tag_fields,
    apply_tools_enabled_field, apply_web_search_tool_mode_field, context_chars_field_value,
    max_output_tokens_field_value, tag_fields, tools_enabled_field, web_search_tool_mode_field,
};
use super::ui::draw_menu;

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
    let mut fields = vec![
        Field::new(t("Config ID", "配置 ID"), provider.id.clone()),
        Field::new(t("Display name", "显示名称"), provider.display_name.clone()),
        Field::new(t("Base URL", "基础地址"), provider.base_url.clone()),
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
            t("Timeout seconds", "超时秒数"),
            provider.timeout_seconds.to_string(),
        ),
        Field::new(
            t("Temperature", "温度参数"),
            provider.temperature.to_string(),
        ),
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
        Field::new(
            t("Client style", "客户端模拟"),
            provider.client_style.clone(),
        )
        .choices(&["auto", "default", "codex"]),
        Field::textarea(
            t("Extra Headers JSON", "自定义请求头 JSON"),
            if provider.extra_headers.is_empty() {
                String::new()
            } else {
                serde_json::to_string_pretty(&provider.extra_headers).unwrap_or_default()
            },
        ),
    ];
    if !run_form(stdout, t(" EDIT PROVIDER ", " 编辑供应商 "), &mut fields)? {
        return Ok(None);
    }
    let extra_body = normalize_extra_body(&fields[9].value)?;
    let extra_headers = normalize_extra_headers(&fields[11].value)?;
    let updated = ProviderConfig {
        id: fields[0].value.trim().to_string(),
        display_name: fields[1].value.trim().to_string(),
        base_url: normalize_base_url(&fields[2].value),
        protocol: fields[3].value.trim().to_string(),
        api_key: Some(fields[4].value.trim().to_string()).filter(|value| !value.is_empty()),
        models: provider.models.clone(),
        model_context_chars: provider.model_context_chars.clone(),
        model_metadata: provider.model_metadata.clone(),
        default_model: provider.default_model.clone(),
        timeout_seconds: fields[5].value.trim().parse().unwrap_or(60),
        temperature: fields[6].value.trim().parse().unwrap_or(0.7),
        anthropic_max_tokens: provider.anthropic_max_tokens,
        thinking_level: fields[7].value.trim().to_string(),
        thinking_format: fields[8].value.trim().to_string(),
        extra_body,
        extra_headers,
        client_style: fields[10].value.trim().to_string(),
    };
    Ok(Some(updated))
}

/// 规范化并校验自定义 Body JSON。
///
/// 参数:
/// - `value`: 表单中输入的 JSON 文本
///
/// 返回:
/// - 为空时返回空字符串，否则返回格式化后的 JSON 对象字符串
/// 规范化自定义请求头 JSON 对象。
///
/// 参数:
/// - `value`: 表单 JSON 文本
///
/// 返回:
/// - 键值表；空输入返回空表
fn normalize_extra_headers(value: &str) -> Result<std::collections::HashMap<String, String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let parsed = serde_json::from_str::<Value>(value)?;
    let obj = parsed.as_object().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            t(
                "Extra Headers JSON must be a JSON object of string values",
                "自定义请求头 JSON 必须是字符串键值对象"
            )
        )
    })?;
    let mut headers = std::collections::HashMap::new();
    for (key, val) in obj {
        let text = match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if !key.trim().is_empty() {
            headers.insert(key.clone(), text);
        }
    }
    Ok(headers)
}

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
    let original = provider.clone();
    let options = vec![
        t("General settings", "常规设置").to_string(),
        t("Model tags", "模型标签").to_string(),
        t("Save model settings", "保存模型设置").to_string(),
    ];
    let mut selected = 0usize;
    loop {
        draw_menu(
            stdout,
            &format!(" EDIT MODEL: {model} "),
            &options,
            selected,
            t("[Enter] open [q] cancel", "[Enter]打开 [q]取消"),
        )?;
        match read_key()? {
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                selected = selected.saturating_sub(1)
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len() - 1)
            }
            crossterm::event::KeyCode::Enter if selected == 0 => {
                edit_model_general_form(stdout, provider, model)?;
            }
            crossterm::event::KeyCode::Enter if selected == 1 => {
                edit_model_tags_form(stdout, provider, model)?;
            }
            crossterm::event::KeyCode::Enter => return Ok(true),
            crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
                *provider = original;
                return Ok(false);
            }
            _ => {}
        }
    }
}

/// 编辑模型常规设置子面板。
fn edit_model_general_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<()> {
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
        Field::new(
            t("Maximum output tokens", "最大输出 token 数"),
            max_output_tokens_field_value(provider, model),
        ),
        web_search_tool_mode_field(provider, model),
    ];
    if !run_form(stdout, t(" MODEL GENERAL ", " 模型常规设置 "), &mut fields)? {
        return Ok(());
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
    apply_max_output_tokens_field(provider, model, &fields[4].value)?;
    apply_web_search_tool_mode_field(provider, model, &fields[5].value);
    Ok(())
}

/// 编辑模型标签子面板。
fn edit_model_tags_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<()> {
    let mut fields = tag_fields(provider, model);
    if run_form(stdout, t(" MODEL TAGS ", " 模型标签 "), &mut fields)? {
        apply_tag_fields(provider, model, &fields)?;
    }
    Ok(())
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

mod advanced;
mod connection;
mod credentials;
mod editor;
pub(crate) mod keys;
mod model_import;
mod values;
use crate::config::ProviderConfig;
use crate::i18n::text as t;
use anyhow::{bail, Result};
pub(crate) use credentials::edit_credentials;
pub(super) use editor::edit_provider_form;
use serde_json::Value;
use std::io;

use super::form::{parse_bool_field, run_form, Field};
use super::input::read_key;
use super::model_metadata_form::{
    apply_deepseek_anchor_mode_field, apply_tag_fields, apply_thinking_level_fields,
    apply_web_search_tool_mode_field, context_chars_field_value, deepseek_anchor_mode_field,
    max_output_tokens_field_value, parse_context_chars_field, parse_max_output_tokens, tag_fields,
    thinking_level_fields, tools_enabled_field, web_search_tool_mode_field,
};
use super::ui::{draw_menu, message};

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
        t("Supported reasoning levels", "支持的推理强度").to_string(),
        t("Save model settings", "保存模型设置").to_string(),
    ];
    let mut selected = 0usize;
    loop {
        draw_menu(
            stdout,
            &format!(" EDIT MODEL: {model} "),
            &options,
            selected,
            &super::theme::help_line(&[("Enter", t("open", "打开")), ("q", t("cancel", "取消"))]),
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
            crossterm::event::KeyCode::Enter if selected == 2 => {
                edit_model_thinking_levels_form(stdout, provider, model)?;
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
        deepseek_anchor_mode_field(provider, model),
    ];
    loop {
        if !run_form(stdout, t(" MODEL GENERAL ", " 模型常规设置 "), &mut fields)? {
            return Ok(());
        }
        // 校验失败时就地提示并重新打开表单，不让非法输入终止 TUI。
        // 先解析全部可失败字段再落地，避免中途报错把 provider 改坏一半
        type ModelGeneralValues = (bool, bool, bool, Option<usize>, Option<u32>);
        let parsed = (|| -> Result<ModelGeneralValues> {
            Ok((
                parse_bool_field(&fields[0].value)?,
                parse_bool_field(&fields[1].value)?,
                parse_bool_field(&fields[2].value)?,
                parse_context_chars_field(&fields[3].value)?,
                parse_max_output_tokens(&fields[4].value)?,
            ))
        })();
        let (active, current, tools_enabled, context_chars, max_output) = match parsed {
            Ok(parsed) => parsed,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
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
        provider.set_model_tools_enabled_for(model, tools_enabled);
        provider.set_model_context_chars_for(model, context_chars);
        provider.set_model_max_output_tokens_for(model, max_output);
        apply_web_search_tool_mode_field(provider, model, &fields[5].value);
        apply_deepseek_anchor_mode_field(provider, model, &fields[6].value);
        return Ok(());
    }
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

/// 编辑模型支持的思考等级子面板。
///
/// 一个都不勾表示不限制，模型目录数据有误时这样恢复。
fn edit_model_thinking_levels_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<()> {
    let mut fields = thinking_level_fields(provider, model);
    if run_form(stdout, t(" REASONING LEVELS ", " 推理强度 "), &mut fields)? {
        apply_thinking_level_fields(provider, model, &fields)?;
    }
    Ok(())
}

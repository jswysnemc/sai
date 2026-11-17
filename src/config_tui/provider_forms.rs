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
use super::model_metadata_form::{
    apply_deepseek_anchor_mode_field, apply_tag_fields, apply_thinking_level_fields,
    apply_web_search_tool_mode_field, context_chars_field_value, deepseek_anchor_mode_field,
    max_output_tokens_field_value, parse_context_chars_field, parse_max_output_tokens, tag_fields,
    thinking_level_fields, tools_enabled_field, web_search_tool_mode_field,
};
use super::ui::message;

/// 【供应商配置】【单屏编辑】把常规参数、模型标签与推理强度收敛到同一个分页表单，取消二次退出丢改动的陷阱。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `provider`: 当前 provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 是否保存修改
pub(super) fn edit_model_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<bool> {
    let active = provider.models.iter().any(|item| item == model);
    let current = provider.default_model == model;
    let context_chars = context_chars_field_value(provider, model);
    let mut fields = vec![
        Field::section(t("General settings", "常规设置")),
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
        Field::section(t("Model tags", "模型标签")),
    ];
    let tags_start = fields.len();
    let tag_list = tag_fields(provider, model);
    let tags_len = tag_list.len();
    fields.extend(tag_list);
    fields.push(Field::section(t(
        "Supported reasoning levels",
        "支持的推理强度",
    )));
    let levels_start = fields.len();
    let level_list = thinking_level_fields(provider, model);
    let levels_len = level_list.len();
    fields.extend(level_list);

    let form_title = format!(" {}: {model} ", t("EDIT MODEL", "编辑模型"));
    loop {
        if !run_form(stdout, &form_title, &mut fields)? {
            return Ok(false);
        }
        // 1. 【供应商配置】【原子解析】先校验全部输入，失败就地提示并不破坏原有数据
        type ModelGeneralValues = (bool, bool, bool, Option<usize>, Option<u32>);
        let parsed = (|| -> Result<ModelGeneralValues> {
            Ok((
                parse_bool_field(&fields[1].value)?,
                parse_bool_field(&fields[2].value)?,
                parse_bool_field(&fields[3].value)?,
                parse_context_chars_field(&fields[4].value)?,
                parse_max_output_tokens(&fields[5].value)?,
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
        apply_web_search_tool_mode_field(provider, model, &fields[6].value);
        apply_deepseek_anchor_mode_field(provider, model, &fields[7].value);
        apply_tag_fields(provider, model, &fields[tags_start..tags_start + tags_len])?;
        apply_thinking_level_fields(
            provider,
            model,
            &fields[levels_start..levels_start + levels_len],
        )?;
        return Ok(true);
    }
}

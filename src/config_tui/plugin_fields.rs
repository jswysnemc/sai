use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;

use super::form::{
    parse_bool_field, parse_provider_model_choice, provider_model_choice_values,
    vision_provider_value, Field,
};
use super::plugins::{plugin_enabled, toggle_plugin};

/// 构造指定 CLI 助手工具的配置字段。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `id`: 当前工具的稳定配置标识
///
/// 返回:
/// - 工具配置表单字段
pub(super) fn plugin_fields(config: &AppConfig, id: &str) -> Vec<Field> {
    match id {
        "vision" => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.vision.enabled),
            Field::new(
                t("Vision Provider/model", "识图 Provider/模型"),
                vision_provider_value(config),
            )
            .choices_owned(provider_model_choice_values(config, true)),
            Field::boolean(
                t("Preview with chafa", "使用 chafa 预览"),
                config.plugins.vision.preview_with_chafa,
            ),
        ],
        "memory" => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.memory.enabled),
            Field::boolean(
                t("Evicted context cache", "上下文弹出缓存"),
                config.plugins.memory.evicted_context_enabled,
            ),
            Field::boolean(
                t("Inject memory index", "注入记忆索引"),
                config.plugins.memory.association_enabled,
            ),
            Field::new(
                t("Evicted snippet chars", "逐出片段字符数"),
                config.plugins.memory.snippet_chars.to_string(),
            ),
        ],
        _ => vec![Field::boolean(
            t("Enabled", "启用"),
            plugin_enabled(config, id),
        )],
    }
}

/// 将 CLI 助手工具表单字段写回配置。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `id`: 当前工具的稳定配置标识
/// - `fields`: 表单字段
///
/// 返回:
/// - 写回是否成功
pub(super) fn apply_plugin_fields(
    config: &mut AppConfig,
    id: &str,
    fields: &[Field],
) -> Result<()> {
    match id {
        "vision" => {
            config.plugins.vision.enabled = parse_bool_field(&fields[0].value)?;
            let (provider_id, model) = parse_provider_model_choice(&fields[1].value);
            config.plugins.vision.vision_provider_id = provider_id;
            config.plugins.vision.vision_model = model;
            config.plugins.vision.preview_with_chafa = parse_bool_field(&fields[2].value)?;
        }
        "memory" => {
            config.plugins.memory.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.memory.evicted_context_enabled = parse_bool_field(&fields[1].value)?;
            config.plugins.memory.association_enabled = parse_bool_field(&fields[2].value)?;
            config.plugins.memory.snippet_chars = fields[3].value.trim().parse::<usize>()?;
        }
        _ => {
            let value = parse_bool_field(&fields[0].value)?;
            if plugin_enabled(config, id) != value {
                toggle_plugin(config, id);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "plugin_fields_tests.rs"]
mod tests;

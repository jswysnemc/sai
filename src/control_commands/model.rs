use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};

use super::ControlSurface;

#[derive(Debug, Clone)]
pub struct ModelCommandResult {
    pub message: String,
    pub changed: bool,
}

/// 执行模型控制命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `selection`: 可选模型序号
/// - `surface`: 命令入口类型
///
/// 返回:
/// - 命令结果
pub fn run_model_command(
    paths: &SaiPaths,
    selection: Option<usize>,
    surface: ControlSurface,
) -> Result<ModelCommandResult> {
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load(paths)?;
    let choices = config.provider_model_choices();
    if choices.is_empty() {
        bail!(
            "{}",
            t(
                "no active provider models; configure or activate a model first",
                "没有已激活的 provider 模型；请先配置或激活模型",
            )
        );
    }

    match selection {
        Some(index) => switch_model_by_index(paths, &mut config, &choices, index),
        None => Ok(ModelCommandResult {
            message: format_model_list(&config, &choices, surface),
            changed: false,
        }),
    }
}

/// 按序号切换模型。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `choices`: 可选模型列表
/// - `index`: 1 基模型序号
///
/// 返回:
/// - 命令结果
fn switch_model_by_index(
    paths: &SaiPaths,
    config: &mut AppConfig,
    choices: &[crate::config::ProviderModelChoice],
    index: usize,
) -> Result<ModelCommandResult> {
    if index == 0 || index > choices.len() {
        bail!(
            "{}: {index}",
            t("model index out of range", "模型序号超出范围")
        );
    }
    let choice = &choices[index - 1];
    let label = choice.label();
    config.set_active_provider_model(&choice.provider_id, &choice.model)?;
    config.save(paths)?;
    Ok(ModelCommandResult {
        message: format!("{}: {index}. {label}", t("active model", "当前模型")),
        changed: true,
    })
}

/// 格式化模型列表。
///
/// 参数:
/// - `config`: 应用配置
/// - `choices`: 可选模型列表
/// - `surface`: 命令入口类型
///
/// 返回:
/// - 模型列表文本
fn format_model_list(
    config: &AppConfig,
    choices: &[crate::config::ProviderModelChoice],
    surface: ControlSurface,
) -> String {
    let active = config.provider(None).ok();
    let mut lines = vec![t("Available models:", "可用模型:").to_string()];
    for (index, choice) in choices.iter().enumerate() {
        let marker = if active
            .map(|provider| {
                provider.id == choice.provider_id && provider.default_model == choice.model
            })
            .unwrap_or(false)
        {
            "*"
        } else {
            " "
        };
        lines.push(format!("{marker} {}. {}", index + 1, choice.label()));
    }
    lines.push(model_switch_hint(surface));
    lines.join("\n")
}

/// 返回模型切换提示。
///
/// 参数:
/// - `surface`: 命令入口类型
///
/// 返回:
/// - 当前入口可用的模型切换提示
fn model_switch_hint(surface: ControlSurface) -> String {
    if surface == ControlSurface::Gateway {
        t(
            "Use /model <index> or /模型 <序号> to switch.",
            "使用 /model <序号> 或 /模型 <序号> 切换。",
        )
        .to_string()
    } else {
        t(
            "Use /model to pick interactively, or /model <index>.",
            "使用 /model 交互选择，或 /model <序号>。",
        )
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_switch_hint_is_surface_specific() {
        let repl = model_switch_hint(ControlSurface::Repl);
        let gateway = model_switch_hint(ControlSurface::Gateway);

        assert!(repl.contains("/model"));
        assert!(!repl.contains("/模型"));
        assert!(gateway.contains("/model"));
        assert!(gateway.contains("/模型"));
    }
}

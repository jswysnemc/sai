//! 首次配置模型选择页：支持 `/` 关键字实时过滤、粘贴检索、两段式 Esc 与手动模型输入。

use super::{
    form::{run_form, Field},
    mouse::{ListMouseState, MenuInput},
    onboarding_ui::{model_footer, model_menu_entry, step_subtitle},
    search::{clamp_index, ListSearch, SearchEffect},
    ui::draw_menu_interactive,
};
use crate::{config::ProviderConfig, i18n::text as t, paths::SaiPaths};
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

/// 【首次配置】【模型过滤】按搜索状态筛选匹配的模型原始下标。
///
/// 参数:
/// - `models`: 远端或预设模型全集
/// - `search`: 列表搜索状态机
///
/// 返回:
/// - 命中过滤条件的模型下标列表
fn filter_model_indices(models: &[String], search: &ListSearch) -> Vec<usize> {
    models
        .iter()
        .enumerate()
        .filter(|(_, model)| search.matches(model))
        .map(|(index, _)| index)
        .collect()
}

/// 【首次配置】【模型选择】查询模型列表供 `/` 检索选择或手动填写模型标识。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: 应用路径
/// - `provider`: 待配置供应商草稿（若成功拉取则顺带回填目录与元数据）
///
/// 返回:
/// - 已确认的模型标识；取消返回上一步时为 None
pub(super) fn select(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    provider: &mut ProviderConfig,
) -> Result<Option<String>> {
    let mut discovery = discover_models(stdout, paths, provider)?;
    loop {
        if discovery.models.is_empty() {
            // 1. 【首次配置】【空目录直达】无目录且无默认模型时直接打开模型标识输入框
            return prompt_custom_model(
                stdout,
                provider,
                t(
                    "No catalog returned · Enter model ID",
                    "未获取到模型列表 · 请填写模型标识",
                ),
            );
        }

        let mut selected = discovery
            .models
            .iter()
            .position(|model| model == &provider.default_model)
            .unwrap_or(0);
        let mut mouse = ListMouseState::default();
        let mut search = ListSearch::default();
        let stage = format!(
            "{} · {}",
            t("Choose model", "选择模型"),
            provider.display_name
        );

        loop {
            // 2. 【首次配置】【实时过滤】按 `/` 搜索词动态过滤模型并始终保留底部手动输入入口
            let visible = filter_model_indices(&discovery.models, &search);
            let mut options = Vec::with_capacity(visible.len().max(1) + 1);
            let mut details = Vec::with_capacity(visible.len().max(1) + 1);
            if visible.is_empty() {
                options.push(format!("  ({})", t("no matches", "没有匹配")));
                details.push(
                    t(
                        "No model matches this filter. Press Esc to clear filter, or Enter to input a custom model ID.",
                        "没有模型命中当前过滤。按 Esc 清空过滤，或按 Enter 手动填写模型标识。",
                    )
                    .to_string(),
                );
            } else {
                for &idx in &visible {
                    let (label, detail) =
                        model_menu_entry(&discovery.models[idx], provider, &discovery.status);
                    options.push(label);
                    details.push(detail);
                }
            }
            options.push(format!("+ {}", t("Custom model ID", "自定义新增模型")));
            details.push(
                t(
                    "Enter any model ID supported by this endpoint manually.",
                    "手动填写该接口支持的任意模型标识。",
                )
                .to_string(),
            );

            selected = clamp_index(selected, options.len());
            let can_confirm = !visible.is_empty() && selected < visible.len();
            let badge = if search.query.is_empty() {
                format!("{} {}", discovery.models.len(), t("models", "个模型"))
            } else {
                format!(
                    "{}/{} {}",
                    visible.len(),
                    discovery.models.len(),
                    t("matches", "项命中")
                )
            };
            let subtitle = step_subtitle(2, 2, &stage, Some(&badge), discovery.fetched);
            let help = search.help().unwrap_or_else(|| model_footer(can_confirm));
            let hitbox = draw_menu_interactive(
                stdout,
                t(" SAI · SELECT MODEL ", " SAI · 选择模型 "),
                &options,
                &details,
                selected,
                &mouse,
                &help,
                &subtitle,
            )?;
            let code = match hitbox.read(&mut mouse, selected, false)? {
                MenuInput::Redraw => continue,
                MenuInput::Paste(text) => {
                    if search.handle_paste(&text) == SearchEffect::Updated {
                        selected = 0;
                    }
                    continue;
                }
                MenuInput::Select(index) | MenuInput::RightClick(index) => {
                    selected = index;
                    continue;
                }
                MenuInput::Activate(index) => {
                    selected = index;
                    KeyCode::Enter
                }
                MenuInput::Action(code) => code,
                MenuInput::Key(key) => key.code,
            };
            match search.handle(code) {
                SearchEffect::Updated => {
                    selected = 0;
                    continue;
                }
                SearchEffect::Closed => continue,
                SearchEffect::Passthrough => {}
            }
            match code {
                KeyCode::Esc | KeyCode::Char('q') if !search.editing => return Ok(None),
                KeyCode::Up | KeyCode::Char('k') if !search.editing => {
                    selected = selected.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') if !search.editing => {
                    selected = clamp_index(selected + 1, options.len())
                }
                KeyCode::Up if search.editing => selected = selected.saturating_sub(1),
                KeyCode::Down if search.editing => {
                    selected = clamp_index(selected + 1, options.len())
                }
                KeyCode::Char('f') if !search.editing => {
                    discovery = discover_models(stdout, paths, provider)?;
                    break;
                }
                KeyCode::Char('m') if !search.editing => {
                    if let Some(model) = prompt_custom_model(
                        stdout,
                        provider,
                        t("Custom model ID", "手动填写自定义模型标识"),
                    )? {
                        return Ok(Some(model));
                    }
                }
                KeyCode::Enter if can_confirm => {
                    return Ok(Some(discovery.models[visible[selected]].clone()))
                }
                KeyCode::Enter => {
                    if let Some(model) = prompt_custom_model(
                        stdout,
                        provider,
                        t("Custom model ID", "手动填写自定义模型标识"),
                    )? {
                        return Ok(Some(model));
                    }
                }
                _ => {}
            }
        }
    }
}

struct CatalogDiscovery {
    models: Vec<String>,
    status: String,
    fetched: bool,
}

/// 【首次配置】【目录拉取】异步拉取模型目录，失败时保留已有预设模型回退。
fn discover_models(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    provider: &mut ProviderConfig,
) -> Result<CatalogDiscovery> {
    let mut draft = provider.clone();
    draft.timeout_seconds = draft.timeout_seconds.clamp(1, 8);
    let paths = paths.clone();
    let result = super::background::run(
        stdout,
        t(" DISCOVER MODELS ", " 获取模型列表 "),
        move || crate::web::services::provider_models::fetch_models(&paths, &draft),
    )?;
    let mut models = Vec::new();
    let mut fetched = false;
    let mut status = match result {
        Some(Ok(result)) => {
            fetched = true;
            for (model, value) in result.metadata {
                provider
                    .model_metadata
                    .entry(model)
                    .or_insert(crate::config::ModelMetadata {
                        context_chars: value.context_chars.and_then(|v| v.try_into().ok()),
                        max_output_tokens: value.max_output_tokens.and_then(|v| v.try_into().ok()),
                        tags: value.tags,
                        thinking_levels: value.thinking_levels,
                        ..Default::default()
                    });
            }
            models = result.models;
            models.sort();
            models.dedup();
            for model in &models {
                if !provider.models.contains(model) {
                    provider.models.push(model.clone());
                }
            }
            t(
                "Choose a discovered model or enter another model ID.",
                "选择接口返回的模型，或手动填写其他模型标识。",
            )
            .to_string()
        }
        _ => t(
            "No live model catalog returned; enter model ID manually.",
            "未获取到接口模型列表，请手动填写模型标识。",
        )
        .to_string(),
    };

    if models.is_empty() && !provider.default_model.trim().is_empty() {
        models = provider.models.clone();
        if !models.contains(&provider.default_model) {
            models.push(provider.default_model.clone());
        }
        models.sort();
        models.dedup();
        status = t(
            "Using configured preset models; confirm default or enter custom ID.",
            "当前展示预设模型；可直接确认默认模型或手动填写标识。",
        )
        .to_string();
    }

    Ok(CatalogDiscovery {
        models,
        status,
        fetched,
    })
}

/// 【首次配置】【手动填写】弹出单步模型输入框，支持鼠标提交和取消返回。
fn prompt_custom_model(
    stdout: &mut io::Stdout,
    provider: &ProviderConfig,
    section: &'static str,
) -> Result<Option<String>> {
    let mut fields = vec![
        Field::section(section),
        Field::new(t("Model ID", "模型标识"), provider.default_model.clone()),
    ];
    loop {
        if !run_form(
            stdout,
            t(" STEP 2/2 · CUSTOM MODEL ", " 步骤 2/2 · 填写模型 "),
            &mut fields,
        )? {
            return Ok(None);
        }
        let model = fields[1].value.trim();
        if !model.is_empty() {
            return Ok(Some(model.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【首次配置测试】【模型过滤】验证模型列表支持 `/` 关键字大小写不敏感检索与清空恢复。
    #[test]
    fn onboarding_model_filter_narrows_and_restores_catalog() {
        let models = vec![
            "anthropic/claude-sonnet-4".to_string(),
            "deepseek/deepseek-v3".to_string(),
            "openai/gpt-4o".to_string(),
        ];
        let mut search = ListSearch::default();
        assert_eq!(filter_model_indices(&models, &search), vec![0, 1, 2]);

        search.handle(KeyCode::Char('/'));
        search.handle_paste("DeepSeek");
        search.handle(KeyCode::Enter);
        assert_eq!(filter_model_indices(&models, &search), vec![1]);

        assert_eq!(search.handle(KeyCode::Esc), SearchEffect::Closed);
        assert_eq!(filter_model_indices(&models, &search), vec![0, 1, 2]);
    }
}

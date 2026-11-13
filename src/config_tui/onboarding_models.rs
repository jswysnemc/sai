//! 首次配置只查询模型目录，不调用推理接口；失败、空目录与取消均可转为手动填写。

use super::{
    form::{run_form, Field},
    input::read_key,
    ui::draw_menu_with_details,
};
use crate::{config::ProviderConfig, i18n::text as t, paths::SaiPaths};
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

/// 【首次配置】【模型选择】参数为输出、应用路径与供应商草稿，返回确认的模型或取消
pub(super) fn select(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    provider: &mut ProviderConfig,
) -> Result<Option<String>> {
    let mut draft = provider.clone();
    draft.timeout_seconds = draft.timeout_seconds.clamp(1, 8);
    let paths = paths.clone();
    let result = super::background::run(
        stdout,
        t(" DISCOVER MODELS ", " 获取模型列表 "),
        move || crate::web::services::provider_models::fetch_models(&paths, &draft),
    )?;
    let mut models = Vec::new();
    let mut status = match result {
        Some(Ok(result)) => {
            for (model, value) in result.metadata {
                provider
                    .model_metadata
                    .entry(model)
                    .or_insert(crate::config::ModelMetadata {
                        context_chars: value.context_chars.and_then(|value| value.try_into().ok()),
                        max_output_tokens: value
                            .max_output_tokens
                            .and_then(|value| value.try_into().ok()),
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
                "Choose a discovered model, or enter another model ID.",
                "选择接口返回的模型，或手动填写其他模型标识。",
            )
        }
        _ => t(
            "No model catalog available. Enter a model ID manually.",
            "未获取到模型列表，可手动填写模型标识。",
        ),
    };
    if models.is_empty() && !provider.default_model.trim().is_empty() {
        // 1. 【首次配置】【目录回退】接口不可用时保留已有模型，内置供应商不强迫用户重新填写模型标识
        models = provider.models.clone();
        if !models.contains(&provider.default_model) {
            models.push(provider.default_model.clone());
        }
        models.sort();
        models.dedup();
        status = t(
            "No live catalog available. Choose a configured model or add a custom ID.",
            "暂未获取到接口模型列表。可选择已有模型或自定义新增模型。",
        );
    }
    let mut options = models.clone();
    options.push(t("Add custom model", "自定义新增模型").into());
    let details = vec![status.to_string(); options.len()];
    let mut selected = models
        .iter()
        .position(|model| model == &provider.default_model)
        .unwrap_or(0);
    loop {
        draw_menu_with_details(
            stdout,
            t(" SELECT MODEL ", " 选择模型 "),
            &options,
            &details,
            selected,
            t(
                "Up/Down select · Enter confirm · Esc back",
                "上下选择 · Enter 确认 · Esc 返回",
            ),
            status,
        )?;
        match read_key()? {
            KeyCode::Esc => return Ok(None),
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter if selected < models.len() => return Ok(Some(models[selected].clone())),
            KeyCode::Enter => {
                let mut fields = vec![Field::new(
                    t("Model ID", "模型标识"),
                    provider.default_model.clone(),
                )];
                if run_form(stdout, t(" CUSTOM MODEL ", " 自定义模型 "), &mut fields)? {
                    let model = fields[0].value.trim();
                    if !model.is_empty() {
                        return Ok(Some(model.into()));
                    }
                }
            }
            _ => {}
        }
    }
}

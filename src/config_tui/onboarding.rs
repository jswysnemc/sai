//! 首次进入聊天前配置供应商，提供免密快捷启动与精简分步引导。

use super::form::run_form;
use super::mouse::{ListMouseState, MenuInput};
use super::onboarding_ui::{
    custom_provider_entry, extract_provider_form, provider_footer, provider_form_fields,
    provider_menu_entry, quick_start_kind, step_subtitle, QuickStartKind,
};
use super::ui::{draw_menu_interactive, message};
use crate::config::onboarding::{complete_provider_setup, ProviderSetupInput};
use crate::config::{AppConfig, ProviderConfig};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io::{self, IsTerminal};

/// 【首次配置】【终端入口】尚未完成配置时打开引导，取消则退出本次聊天启动。
///
/// 参数:
/// - `paths`: 应用路径
///
/// 返回:
/// - 可以继续进入聊天时为 true；用户取消时为 false
pub(crate) fn ensure_provider_setup(paths: &SaiPaths) -> Result<bool> {
    let config = AppConfig::load_or_default(paths)?;
    if config.provider_setup_complete || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(true);
    }
    super::session::run_provider_setup(paths, &config)
}

/// 【首次配置】【供应商菜单】展示就绪状态，支持免密一键启动与定制编辑，完成后结束引导。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: 应用路径
/// - `config`: 当前配置
///
/// 返回:
/// - 已完成时为 true；退出时为 false
pub(super) fn run(stdout: &mut io::Stdout, paths: &SaiPaths, config: &AppConfig) -> Result<bool> {
    // 1. 预先分类各模板的凭据就绪度，用于状态标签、详情说明与零表单快捷启动
    let kinds: Vec<QuickStartKind> = config
        .providers
        .iter()
        .map(|provider| quick_start_kind(provider, paths))
        .collect();
    let ready_count = kinds
        .iter()
        .filter(|kind| matches!(kind, QuickStartKind::BuiltInFree | QuickStartKind::EnvReady))
        .count();
    let ready_badge =
        (ready_count > 0).then(|| format!("{ready_count} {}", t("ready to start", "个已就绪")));

    let mut options = Vec::with_capacity(config.providers.len() + 1);
    let mut details = Vec::with_capacity(config.providers.len() + 1);
    for (provider, kind) in config.providers.iter().zip(&kinds) {
        let (label, detail) = provider_menu_entry(provider, kind);
        options.push(label);
        details.push(detail);
    }
    let (custom_label, custom_detail) = custom_provider_entry();
    options.push(custom_label);
    details.push(custom_detail);

    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    loop {
        let current_kind = kinds.get(selected);
        let subtitle = step_subtitle(
            1,
            2,
            t("Choose provider", "选择供应商"),
            ready_badge.as_deref(),
            true,
        );
        let hitbox = draw_menu_interactive(
            stdout,
            t(" SAI · WELCOME SETUP ", " SAI · 首次使用引导 "),
            &options,
            &details,
            selected,
            &mouse,
            &provider_footer(current_kind),
            &subtitle,
        )?;
        let code = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw => continue,
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

        let existing = selected < config.providers.len();
        let selected_provider = config
            .providers
            .get(selected)
            .cloned()
            .unwrap_or_else(ProviderConfig::new_openai_compatible);

        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter if matches!(current_kind, Some(QuickStartKind::BuiltInFree)) => {
                // 2. 内置免费供应商直接一键启用，减少多余表单和空密钥困惑
                let model = selected_provider.default_model.clone();
                if finish_setup(stdout, paths, selected_provider, existing, model)? {
                    return Ok(true);
                }
            }
            KeyCode::Char(' ' | 's')
                if matches!(
                    current_kind,
                    Some(QuickStartKind::BuiltInFree | QuickStartKind::EnvReady)
                ) =>
            {
                let model = selected_provider.default_model.clone();
                if finish_setup(stdout, paths, selected_provider, existing, model)? {
                    return Ok(true);
                }
            }
            KeyCode::Char('m')
                if existing
                    && matches!(
                        current_kind,
                        Some(QuickStartKind::BuiltInFree | QuickStartKind::EnvReady)
                    ) =>
            {
                if select_model_and_finish(stdout, paths, selected_provider, true)? {
                    return Ok(true);
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if configure(stdout, paths, &selected_provider, existing)? {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
}

/// 【首次配置】【连接表单】收集接入信息并跳转模型选择，校验失败保留输入并提示。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: 应用路径
/// - `provider`: 模板或自定义基础配置
/// - `existing`: 是否复用内置供应商模板
///
/// 返回:
/// - 保存成功时为 true；取消返回上一步时为 false
fn configure(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    provider: &ProviderConfig,
    existing: bool,
) -> Result<bool> {
    let mut fields = provider_form_fields(provider, existing);
    let title = if existing {
        format!(
            " {} · {} ",
            t("STEP 1/2", "步骤 1/2"),
            provider.display_name
        )
    } else {
        t(" STEP 1/2 · CUSTOM PROVIDER ", " 步骤 1/2 · 自定义供应商 ").to_string()
    };
    loop {
        if !run_form(stdout, &title, &mut fields)? {
            return Ok(false);
        }
        let (display_name, base_url, protocol, api_key) =
            extract_provider_form(&fields, existing, provider.is_opencode_zen());
        let mut draft = provider.clone();
        draft.display_name = display_name;
        draft.base_url = base_url;
        draft.protocol = protocol;
        if !api_key.trim().is_empty() {
            draft.api_key = Some(api_key);
            draft.api_keys.clear();
            draft.api_key_selected = None;
        }
        if select_model_and_finish(stdout, paths, draft, existing)? {
            return Ok(true);
        }
    }
}

/// 【首次配置】【模型步衔接】先选择默认模型再持久化完整供应商配置。
fn select_model_and_finish(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    mut draft: ProviderConfig,
    existing: bool,
) -> Result<bool> {
    let Some(model) = super::onboarding_models::select(stdout, paths, &mut draft)? else {
        return Ok(false);
    };
    finish_setup(stdout, paths, draft, existing, model)
}

/// 【首次配置】【持久化提交】写入供应商、默认模型及发现的模型目录元数据。
fn finish_setup(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    draft: ProviderConfig,
    existing: bool,
    model: String,
) -> Result<bool> {
    let input = ProviderSetupInput {
        provider_id: existing.then(|| draft.id.clone()),
        display_name: draft.display_name.clone(),
        base_url: draft.base_url.clone(),
        protocol: draft.protocol.clone(),
        api_key: draft.api_key.clone(),
        model,
    };
    match complete_provider_setup(paths, input) {
        Ok(mut saved) => {
            let active = saved.active_provider.clone();
            if let Some(provider) = saved
                .providers
                .iter_mut()
                .find(|provider| provider.id == active)
            {
                for discovered in draft.models {
                    if !provider.models.contains(&discovered) {
                        provider.models.push(discovered);
                    }
                }
                provider.model_metadata.extend(draft.model_metadata);
            }
            saved.save(paths)?;
            Ok(true)
        }
        Err(error) => {
            message(
                stdout,
                &format!(
                    "{}: {error}",
                    t("Could not save provider", "无法保存供应商")
                ),
            )?;
            Ok(false)
        }
    }
}

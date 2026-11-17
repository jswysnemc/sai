//! 激活供应商与模型快速选择器：支持 `/` 搜索、粘贴过滤及 `d` 键二次确认移除模型。

use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

use super::super::mouse::{ListMouseState, MenuInput};
use super::super::search::{clamp_index, ListSearch, SearchEffect};
use super::super::ui::{confirm_delete, draw_menu_interactive, message};

/// 【供应商配置】【快速选择激活项】从所有可用供应商模型中选择默认模型，支持检索与安全删除。
///
/// 参数:
/// - `stdout`: 终端输出流
/// - `config`: 待修改的应用配置
///
/// 返回:
/// - 交互选择执行结果
pub(crate) fn select_active_provider(
    stdout: &mut io::Stdout,
    config: &mut AppConfig,
) -> Result<()> {
    let mut choices = config.provider_model_choices();
    if choices.is_empty() {
        message(
            stdout,
            t(
                "No available Provider, add one first.",
                "没有可用 Provider，请先添加。",
            ),
        )?;
        return Ok(());
    }
    let current = choices.iter().position(|choice| {
        config
            .provider(None)
            .map(|provider| {
                provider.id == choice.provider_id && provider.default_model == choice.model
            })
            .unwrap_or(false)
    });
    let mut selected = current.unwrap_or(0);
    let mut mouse = ListMouseState::default();
    let mut search = ListSearch::default();
    let mut status = String::new();
    loop {
        if choices.is_empty() {
            message(
                stdout,
                t(
                    "No available Provider models left.",
                    "已无可用 Provider 模型。",
                ),
            )?;
            return Ok(());
        }
        let visible = choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| search.matches(&choice.label()))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        selected = clamp_index(selected, visible.len());
        let options = if visible.is_empty() {
            vec![format!("({})", t("no matches", "没有匹配"))]
        } else {
            visible
                .iter()
                .map(|index| choices[*index].label())
                .collect::<Vec<_>>()
        };
        let help = search.help().unwrap_or_else(|| {
            if status.is_empty() {
                super::super::theme::help_line(&[
                    ("/", t("search", "搜索")),
                    ("↑↓", t("move", "移动")),
                    ("Enter", t("select", "选择")),
                    ("d", t("remove", "移除")),
                    ("q", t("back", "返回")),
                ])
            } else {
                status.clone()
            }
        });
        let hitbox = draw_menu_interactive(
            stdout,
            t(" SELECT PROVIDER/MODEL ", " 选择供应商/模型 "),
            &options,
            &[],
            selected,
            &mouse,
            &help,
            "",
        )?;
        let key = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw => continue,
            MenuInput::Paste(text) => {
                if search.handle_paste(&text) == SearchEffect::Updated {
                    selected = 0;
                    status.clear();
                }
                continue;
            }
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                if !visible.is_empty() {
                    selected = index;
                }
                continue;
            }
            MenuInput::Activate(index) => {
                if !visible.is_empty() {
                    selected = index;
                }
                KeyCode::Enter
            }
            MenuInput::Key(key) => key.code,
            MenuInput::Action(code) => code,
        };
        match search.handle(key) {
            SearchEffect::Updated => {
                selected = 0;
                status.clear();
                continue;
            }
            SearchEffect::Closed => continue,
            SearchEffect::Passthrough => {}
        }
        match key {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') if !search.editing => {
                selected = selected.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j') if !search.editing => {
                selected = clamp_index(selected + 1, visible.len())
            }
            KeyCode::Up if search.editing => selected = selected.saturating_sub(1),
            KeyCode::Down if search.editing => selected = clamp_index(selected + 1, visible.len()),
            KeyCode::Char('d') if !visible.is_empty() => {
                let choice = &choices[visible[selected]];
                let provider_id = choice.provider_id.clone();
                let model = choice.model.clone();
                let target_label = choice.label();
                // 1. 【供应商配置】【二次确认】移除模型与关联元数据前弹出危险操作确认框防止误触
                if !confirm_delete(
                    stdout,
                    t(" REMOVE MODEL ", " 移除模型 "),
                    &target_label,
                    t(
                        "Removes this model and its associated metadata from the provider.",
                        "从该供应商中移除此模型及其关联元数据。",
                    ),
                )? {
                    status = t("Delete cancelled", "已取消删除").to_string();
                    continue;
                }
                match config.remove_active_provider_model(&provider_id, &model) {
                    Ok(()) => {
                        status = format!("{}: {model}", t("Removed model", "已移除模型"));
                        choices = config.provider_model_choices();
                    }
                    Err(err) => status = err.to_string(),
                }
            }
            KeyCode::Enter if !visible.is_empty() => {
                let choice = &choices[visible[selected]];
                config.set_active_provider_model(&choice.provider_id, &choice.model)?;
                return Ok(());
            }
            _ => {}
        }
    }
}

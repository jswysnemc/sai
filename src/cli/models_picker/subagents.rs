mod render;
mod state;
#[cfg(test)]
mod tests;

use super::{clear_frame, draw_at, read_key};
use crate::config::{AppConfig, SubagentModelChoice};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use state::{choice_state, SubagentListState};
use std::io;

/// 【终端】【子任务模型】配置共享默认值或单个任务类型的模型与思考等级。
///
/// 参数: `stdout` 为输出，`anchor_y` 和 `frame_rows` 定位面板，`config` 与 `paths` 用于保存
/// 返回: 配置交互结果；已确认的选择立即写入磁盘
pub(super) fn run(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    config: &mut AppConfig,
    paths: &SaiPaths,
) -> Result<()> {
    let mut focus_id = None;
    let mut status = String::new();
    loop {
        let mut state = SubagentListState::new(config, focus_id.as_deref());
        let selected = loop {
            draw_at(
                stdout,
                anchor_y,
                frame_rows,
                &render::list(&state, frame_rows, &status),
            )?;
            let key = read_key()?;
            match key.code {
                KeyCode::Esc => {
                    clear_frame(stdout, anchor_y, frame_rows)?;
                    return Ok(());
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(())
                }
                KeyCode::Enter => break state.selected().clone(),
                KeyCode::Up => state.move_up(),
                KeyCode::Down => state.move_down(),
                _ => {}
            }
        };
        focus_id = selected.profile_id.clone();
        let Some(choice) = choose(stdout, anchor_y, frame_rows, config, &selected)? else {
            continue;
        };
        match config.set_subagent_model_choice(selected.profile_id.as_deref(), choice) {
            Ok(()) => {
                config.save(paths)?;
                status = t(
                    "Saved · applies to new subagents",
                    "已保存，后续启动的子任务生效",
                )
                .to_string();
            }
            Err(error) => status = error.to_string(),
        }
    }
}

/// 【终端】【子任务模型】在模型和思考两列中选择一个任务类型的运行参数。
///
/// 参数: `stdout`、`anchor_y`、`frame_rows` 为面板输出，`config` 为模型来源，`target` 为编辑对象
/// 返回: 确认后的选择；退出时返回空
fn choose(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    config: &AppConfig,
    target: &state::SubagentTarget,
) -> Result<Option<SubagentModelChoice>> {
    let mut picker = choice_state(config, target);
    loop {
        let mut lines = super::render::render(&picker);
        lines[0] = format!(
            "{}{} · {}{}",
            super::render::FOCUS_STYLE,
            t("Subagent settings", "子任务设置"),
            target.name,
            super::render::RESET
        );
        if let Some(footer) = lines.last_mut() {
            *footer = format!(
                "{}{}{}",
                super::render::DIM_STYLE,
                t(
                    "Type to filter · ↑/↓ choose · ←/→ model/thinking · Enter save · Esc back",
                    "输入过滤 · ↑/↓ 选择 · ←/→ 模型/思考 · Enter 保存 · Esc 返回"
                ),
                super::render::RESET
            );
        }
        draw_at(stdout, anchor_y, frame_rows, &lines)?;
        let key = read_key()?;
        match key.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(None),
            KeyCode::Enter => {
                if let Some(model) = picker.selected_model() {
                    return Ok(Some(SubagentModelChoice {
                        provider_id: model.provider_id.clone(),
                        model: model.model.clone(),
                        thinking_level: picker.selected_level().to_string(),
                    }));
                }
            }
            KeyCode::Up => picker.move_up(),
            KeyCode::Down => picker.move_down(),
            KeyCode::Left => picker.focus_model(),
            KeyCode::Right => picker.focus_thinking(),
            KeyCode::Tab => match picker.column() {
                super::state::PickerColumn::Model => picker.focus_thinking(),
                super::state::PickerColumn::Thinking => picker.focus_model(),
            },
            KeyCode::Backspace => picker.pop_filter(),
            KeyCode::Delete => picker.clear_filter(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                picker.clear_filter()
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                picker.push_filter(ch)
            }
            _ => {}
        }
    }
}

mod render;
mod state;
#[cfg(test)]
mod tests;

use super::mouse::PickerFrame;
use super::{clear_frame, draw_at};
use crate::cli::mouse_menu::ClickTracker;
use crate::config::{AppConfig, SubagentModelChoice};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind, MouseEventKind};
use crossterm::event::{KeyCode, KeyModifiers};
use state::{choice_state, SubagentListState};
use std::io;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ListTarget {
    Item(usize),
    Back,
}

/// 【终端】【子任务模型】配置共享默认值或单个任务类型的模型与思考等级。
///
/// 参数: `stdout` 为输出，`frame` 定位面板，`config` 与 `paths` 用于保存
/// 返回: 配置交互结果；已确认的选择立即写入磁盘
pub(super) fn run(
    stdout: &mut io::Stdout,
    frame: &mut PickerFrame,
    config: &mut AppConfig,
    paths: &SaiPaths,
) -> Result<()> {
    let mut click = ClickTracker::default();
    let mut focus_id = None;
    let mut status = String::new();
    loop {
        let mut state = SubagentListState::new(config, focus_id.as_deref());
        let selected = loop {
            draw_at(
                stdout,
                frame.anchor,
                frame.rows,
                &render::list(&state, frame.rows, &status),
            )?;
            let key = match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => {
                    click.clear();
                    key
                }
                Event::Resize(columns, rows) => {
                    click.clear();
                    frame.resize(columns, rows)?;
                    continue;
                }
                Event::Mouse(mouse) => {
                    let content = render::content_rows(usize::from(frame.rows));
                    let start = render::window_start(&state, content.len());
                    let row = mouse.row.checked_sub(frame.anchor).map(usize::from);
                    let target = row.and_then(|row| {
                        if row == usize::from(frame.rows.saturating_sub(1)) {
                            return render::controls(frame.width).hit(mouse.column);
                        }
                        (content.contains(&row) && usize::from(mouse.column) < frame.width)
                            .then_some(start + row.saturating_sub(content.start))
                            .filter(|index| *index < state.targets.len())
                            .map(ListTarget::Item)
                    });
                    let selected = click.update(mouse, target);
                    if let Some(target) = selected {
                        match target {
                            ListTarget::Back => KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                            ListTarget::Item(index) => {
                                state.index = index;
                                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
                            }
                        }
                    } else {
                        if row.is_some_and(|row| content.contains(&row)) {
                            match mouse.kind {
                                MouseEventKind::ScrollUp => state.move_up(),
                                MouseEventKind::ScrollDown => state.move_down(),
                                _ => {}
                            }
                        }
                        continue;
                    }
                }
                _ => continue,
            };
            match key.code {
                KeyCode::Esc => {
                    clear_frame(stdout, frame.anchor, frame.rows)?;
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
        let Some(choice) = choose(stdout, frame, config, &selected)? else {
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
/// 参数: `stdout`、`frame` 为面板输出，`config` 为模型来源，`target` 为编辑对象
/// 返回: 确认后的选择；退出时返回空
fn choose(
    stdout: &mut io::Stdout,
    frame: &mut PickerFrame,
    config: &AppConfig,
    target: &state::SubagentTarget,
) -> Result<Option<SubagentModelChoice>> {
    let mut picker = choice_state(config, target);
    let mut click = Default::default();
    loop {
        let mut lines =
            super::render::render_sized(&picker, frame.width, usize::from(frame.rows), false);
        if frame.rows >= 7 {
            lines[0] = format!(
                "{}{} · {}{}",
                super::render::FOCUS_STYLE,
                t("Subagent settings", "子任务设置"),
                target.name,
                super::render::RESET
            );
        }
        draw_at(stdout, frame.anchor, frame.rows, &lines)?;
        let Some(key) = frame.read(&mut picker, &mut click, false)? else {
            continue;
        };
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
                super::state::PickerColumn::Thinking | super::state::PickerColumn::Context => {
                    picker.focus_model()
                }
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

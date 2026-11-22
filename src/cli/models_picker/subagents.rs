mod render;
mod state;
#[cfg(test)]
mod tests;

use super::mouse::{PickerFrame, PickerInput};
use super::{clear_frame, draw_at};
use crate::cli::mouse_menu::ClickTracker;
use crate::config::{AppConfig, SubagentModelChoice};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
};
use state::{choice_state, SubagentListState};
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListTarget {
    Item(usize),
    Back,
}

/// 子任务列表鼠标事件的处理结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubagentMouseAction {
    /// 无可见状态变化
    Ignore,
    /// 悬停或焦点行更新，需要重绘列表
    Redraw,
    /// 触发编辑进入或返回上一级
    Key(KeyEvent),
}

/// 子任务二级模型选择器的退出结果。
enum ChoiceOutcome {
    /// 已确认选择新的模型与思考等级
    Selected(SubagentModelChoice),
    /// 按 Esc 返回子任务列表
    Back,
    /// 按 Ctrl+C 中断并直接退出整个模型选择器
    Interrupted,
}

/// 【终端】【子任务模型】配置共享默认值或单个任务类型的模型与思考等级。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `frame`: 共享内联面板定位帧
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 用户按 `Ctrl+C` 请求退出整个模型选择器时返回 true，按 `Esc` 返回主面板时返回 false
pub(super) fn run(
    stdout: &mut io::Stdout,
    frame: &mut PickerFrame,
    config: &mut AppConfig,
    paths: &SaiPaths,
) -> Result<bool> {
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
                &render::list(&state, frame.width, frame.rows, &status),
                None,
            )?;
            let Some(key) = read_list_key(&mut state, frame, &mut click)? else {
                continue;
            };
            match key.code {
                KeyCode::Esc => {
                    clear_frame(stdout, frame.anchor, frame.rows)?;
                    return Ok(false);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // 1. 【子任务设置】【中断清帧】Ctrl+C 立即擦除帧并通知外层直接结束，无需连按三次
                    clear_frame(stdout, frame.anchor, frame.rows)?;
                    return Ok(true);
                }
                KeyCode::Enter => break state.selected().clone(),
                KeyCode::Up => state.move_up(),
                KeyCode::Down => state.move_down(),
                _ => {}
            }
        };
        focus_id = selected.profile_id.clone();
        let choice = match choose(stdout, frame, config, &selected)? {
            ChoiceOutcome::Selected(choice) => choice,
            ChoiceOutcome::Back => continue,
            ChoiceOutcome::Interrupted => {
                clear_frame(stdout, frame.anchor, frame.rows)?;
                return Ok(true);
            }
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
/// 参数:
/// - `stdout`: 终端输出
/// - `frame`: 面板定位帧
/// - `config`: 模型候选来源配置
/// - `target`: 当前编辑的子任务目标
///
/// 返回:
/// - 确认选择、返回列表或中断退出结果
fn choose(
    stdout: &mut io::Stdout,
    frame: &mut PickerFrame,
    config: &AppConfig,
    target: &state::SubagentTarget,
) -> Result<ChoiceOutcome> {
    let mut picker = choice_state(config, target);
    let mut click = Default::default();
    loop {
        let theme = super::render::PickerTheme::current();
        let mut lines =
            super::render::render_sized(&picker, frame.width, usize::from(frame.rows), false);
        if frame.rows >= 7 {
            lines[0] = format!(
                "{}{} · {}{}",
                theme.focus,
                t("Subagent settings", "子任务设置"),
                target.name,
                super::render::RESET
            );
        }
        let cursor = super::render::input_cursor_position(
            &picker,
            frame.width,
            usize::from(frame.rows),
        );
        draw_at(stdout, frame.anchor, frame.rows, &lines, cursor)?;
        let Some(input) = frame.read(&mut picker, &mut click, false)? else {
            continue;
        };
        let key = match input {
            PickerInput::Paste(text) => {
                picker.paste_filter(&text);
                continue;
            }
            PickerInput::Key(key) => key,
        };
        match key.code {
            KeyCode::Esc => return Ok(ChoiceOutcome::Back),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(ChoiceOutcome::Interrupted)
            }
            KeyCode::Enter => {
                if let Some(model) = picker.selected_model() {
                    return Ok(ChoiceOutcome::Selected(SubagentModelChoice {
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

/// 【子任务设置】【列表事件读取】读取键盘、鼠标、失焦与窗口缩放事件，按需请求重绘。
///
/// 参数:
/// - `state`: 子任务列表状态
/// - `frame`: 共享内联面板定位帧
/// - `click`: 单击跟踪器
///
/// 返回:
/// - 需交由列表主循环处理的按键事件，返回空表示仅需重绘
fn read_list_key(
    state: &mut SubagentListState,
    frame: &mut PickerFrame,
    click: &mut ClickTracker<ListTarget>,
) -> Result<Option<KeyEvent>> {
    loop {
        match crate::cli::terminal_events::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                click.clear();
                state.set_hovered(None);
                return Ok(Some(key));
            }
            Event::FocusLost => {
                click.clear();
                if state.set_hovered(None) {
                    return Ok(None);
                }
            }
            Event::Resize(columns, rows) => {
                click.clear();
                state.set_hovered(None);
                frame.resize(columns, rows)?;
                return Ok(None);
            }
            Event::Mouse(mouse) => match handle_list_mouse(
                state,
                click,
                frame.anchor,
                frame.rows,
                frame.width,
                mouse,
            ) {
                SubagentMouseAction::Ignore => {}
                SubagentMouseAction::Redraw => return Ok(None),
                SubagentMouseAction::Key(key) => return Ok(Some(key)),
            },
            _ => {}
        }
    }
}

/// 【子任务设置】【列表鼠标映射】计算子任务行及返回按钮的悬停、聚焦、二次点击进入与滚轮滚动。
///
/// 参数:
/// - `state`: 子任务列表状态
/// - `click`: 单击跟踪器
/// - `anchor`: 面板首行终端行号
/// - `frame_rows`: 面板占用总行数
/// - `width`: 面板可用列宽
/// - `mouse`: 终端鼠标事件
///
/// 返回:
/// - 忽略、重绘或按键动作
fn handle_list_mouse(
    state: &mut SubagentListState,
    click: &mut ClickTracker<ListTarget>,
    anchor: u16,
    frame_rows: u16,
    width: usize,
    mouse: MouseEvent,
) -> SubagentMouseAction {
    // 1. 【子任务设置】【边界检查】指针离开面板矩形时清空悬停与未完成点击
    let in_rows = mouse
        .row
        .checked_sub(anchor)
        .map(usize::from)
        .filter(|&row| row < usize::from(frame_rows));
    let Some(row) = in_rows.filter(|_| usize::from(mouse.column) < width) else {
        if mouse.kind != MouseEventKind::Moved {
            click.clear();
        }
        return if state.set_hovered(None) {
            SubagentMouseAction::Redraw
        } else {
            SubagentMouseAction::Ignore
        };
    };

    // 2. 【子任务设置】【目标命中】匹配底栏返回按钮或当前可视窗口内的子任务行
    let content = render::content_rows(usize::from(frame_rows));
    let hit_at = |list_state: &SubagentListState| -> Option<ListTarget> {
        if row == usize::from(frame_rows.saturating_sub(1)) {
            return render::controls(width).hit(mouse.column);
        }
        let start = render::window_start(list_state, content.len());
        content
            .contains(&row)
            .then_some(start + row.saturating_sub(content.start))
            .filter(|&index| index < list_state.targets.len())
            .map(ListTarget::Item)
    };
    let target = hit_at(state);
    let changed = state.set_hovered(target);

    // 3. 【子任务设置】【单击处理】未选中行单击先聚焦，已选中行再次单击进入二级模型设置
    if let Some(clicked) = click.update(mouse, target) {
        match clicked {
            ListTarget::Back => {
                return SubagentMouseAction::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            }
            ListTarget::Item(index) => {
                if state.index == index {
                    return SubagentMouseAction::Key(KeyEvent::new(
                        KeyCode::Enter,
                        KeyModifiers::NONE,
                    ));
                }
                state.index = index;
                state.set_hovered(hit_at(state));
                return SubagentMouseAction::Redraw;
            }
        }
    }

    // 4. 【子任务设置】【滚轮导航】在内容区内滚动列表并同步指针下的新悬停项
    if content.contains(&row)
        && matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        )
    {
        if mouse.kind == MouseEventKind::ScrollUp {
            state.move_up();
        } else {
            state.move_down();
        }
        state.set_hovered(hit_at(state));
        return SubagentMouseAction::Redraw;
    }

    if changed {
        SubagentMouseAction::Redraw
    } else {
        SubagentMouseAction::Ignore
    }
}

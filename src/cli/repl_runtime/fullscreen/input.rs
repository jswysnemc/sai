//! 全屏会话视图的输入分流：鼠标与浏览键由全屏消费，其余按键交回输入框。

use super::overview::{mark_at_row, rail_marks};
use super::paint::scroll_for_track_row;
use super::state::WHEEL_STEP;
use crate::cli::repl_runtime::ReplRuntime;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// 全屏对一个终端事件的处理结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli) enum FullscreenEvent {
    /// 事件已由全屏视图处理
    Consumed,
    /// 交回输入框按原逻辑处理
    Pass,
}

/// 一次交互造成的变化。
enum Effect {
    /// 视图位置或展开状态变化，需要重绘
    Repaint,
    /// 已处理但画面不变
    Nothing,
    /// 不属于全屏视图
    Pass,
}

impl ReplRuntime {
    /// 【全屏视图】【输入分流】先让全屏视图处理事件；非全屏时一律交回。
    ///
    /// 参数:
    /// - `event`: 终端事件
    ///
    /// 返回:
    /// - 是否已被全屏视图消费
    pub(in crate::cli) fn handle_fullscreen_event(
        &mut self,
        event: &Event,
    ) -> Result<FullscreenEvent> {
        if self.fullscreen.is_none() {
            return Ok(FullscreenEvent::Pass);
        }
        let effect = match event {
            Event::Mouse(mouse) => self.fullscreen_mouse(*mouse),
            Event::Key(key) if key.kind != KeyEventKind::Release => self.fullscreen_key(*key),
            _ => Effect::Pass,
        };
        match effect {
            Effect::Repaint => {
                self.paint_fullscreen()?;
                Ok(FullscreenEvent::Consumed)
            }
            Effect::Nothing => Ok(FullscreenEvent::Consumed),
            Effect::Pass => Ok(FullscreenEvent::Pass),
        }
    }

    /// 处理浏览键：翻页、切换用户消息、首尾跳转。
    ///
    /// 参数:
    /// - `key`: 按键事件
    ///
    /// 返回:
    /// - 处理效果
    fn fullscreen_key(&mut self, key: KeyEvent) -> Effect {
        let Some(session) = self.fullscreen.as_mut() else {
            return Effect::Pass;
        };
        let Some(layout) = session.state.layout else {
            return Effect::Pass;
        };
        let height = usize::from(layout.body_height);
        let page = height.saturating_sub(2).max(1) as isize;
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let state = &mut session.state;
        let changed = match key.code {
            KeyCode::PageUp => state.scroll_by(-page, height),
            KeyCode::PageDown => state.scroll_by(page, height),
            KeyCode::Up if alt => state.jump_relative(false, height),
            KeyCode::Down if alt => state.jump_relative(true, height),
            KeyCode::Home if ctrl => state.scroll_to(0, height),
            KeyCode::End if ctrl => state.scroll_to(usize::MAX, height),
            // Ctrl+↓ 与内联模式一致：回到最新输出
            KeyCode::Down if ctrl => state.scroll_to(usize::MAX, height),
            _ => return Effect::Pass,
        };
        if changed {
            Effect::Repaint
        } else {
            Effect::Nothing
        }
    }

    /// 处理鼠标：滚轮滚动、点击段落展开收起、概览跳转与悬停、滚动条拖动。
    ///
    /// 参数:
    /// - `mouse`: 鼠标事件
    ///
    /// 返回:
    /// - 处理效果
    fn fullscreen_mouse(&mut self, mouse: MouseEvent) -> Effect {
        let Some(session) = self.fullscreen.as_mut() else {
            return Effect::Pass;
        };
        let Some(layout) = session.state.layout else {
            return Effect::Nothing;
        };
        let height = usize::from(layout.body_height);
        let body_row = usize::from(mouse.row.saturating_sub(layout.body_top));
        let state = &mut session.state;
        let changed = match mouse.kind {
            MouseEventKind::ScrollUp => state.scroll_by(-(WHEEL_STEP as isize), height),
            MouseEventKind::ScrollDown => state.scroll_by(WHEEL_STEP as isize, height),
            // 1. 悬停：只在指向的概览标记变化时重绘
            MouseEventKind::Moved => {
                let hover = if layout.in_body(mouse.row) && layout.in_rail(mouse.column) {
                    let marks = rail_marks(state.document.anchors.len(), height);
                    mark_at_row(&marks, body_row).map(|mark| mark.anchor)
                } else {
                    None
                };
                let changed = hover != state.hover;
                state.hover = hover;
                changed
            }
            MouseEventKind::Drag(MouseButton::Left) if state.dragging => {
                let target = scroll_for_track_row(state, body_row, height);
                state.scroll_to(target, height)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                state.dragging = false;
                false
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // 2. 标题：点“新输出”到底，点其余位置回到当前消息开头
                if mouse.row < layout.body_top {
                    let on_unseen = session
                        .unseen_cols
                        .is_some_and(|(start, end)| mouse.column >= start && mouse.column < end);
                    if on_unseen {
                        state.scroll_to(usize::MAX, height)
                    } else if let Some(index) = state.current_anchor() {
                        state.jump_to_anchor(index, height)
                    } else {
                        false
                    }
                } else if !layout.in_body(mouse.row) {
                    // 输入框区域：交回终端，不做处理
                    false
                } else if mouse.column == layout.scrollbar_col {
                    // 3. 滚动条：按点击位置定位并开始拖动
                    state.dragging = true;
                    let target = scroll_for_track_row(state, body_row, height);
                    state.scroll_to(target, height)
                } else if layout.in_rail(mouse.column) {
                    // 4. 概览标记：跳到对应用户消息
                    let marks = rail_marks(state.document.anchors.len(), height);
                    match mark_at_row(&marks, body_row) {
                        Some(mark) => state.jump_to_anchor(mark.anchor, height),
                        None => false,
                    }
                } else if mouse.column < layout.content_width {
                    // 5. 正文：点击可折叠段落展开或收起
                    match state.toggle_at(state.scroll + body_row) {
                        Some(anchor) => {
                            session.pending_toggle = Some(anchor);
                            true
                        }
                        None => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        };
        if changed {
            Effect::Repaint
        } else {
            Effect::Nothing
        }
    }
}

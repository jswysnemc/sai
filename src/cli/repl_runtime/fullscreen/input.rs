//! 全屏会话视图的输入分流：鼠标与浏览键由全屏消费，其余按键交回输入框。

use super::overview::{mark_at_row, rail_marks};
use super::paint::scroll_for_track_row;
use super::selection;
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
        // 1. 选区复制经 OSC 52 写入终端剪贴板，随下一帧一起送出
        if let Some(text) = self
            .fullscreen
            .as_mut()
            .and_then(|session| session.pending_copy.take())
        {
            use std::io::Write as _;
            write!(self.frame, "{}", super::selection::osc52_copy(&text))?;
            self.commit_frame()?;
        }
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
        // 1. 【全屏视图】【待办切换】只响应可见 TODO 标题，不把输入区或被裁掉的标题当作按钮
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            && self
                .transcript
                .latest_todo_items()
                .iter()
                .any(|item| matches!(item.status.as_str(), "pending" | "in_progress"))
            && self
                .last_composer_signature
                .as_ref()
                .is_some_and(|signature| signature.hits_panel_header(mouse.column, mouse.row))
        {
            if let Some(session) = self.fullscreen.as_mut() {
                session.state.press = None;
                session.state.selection = None;
                session.state.copied = None;
                session.state.dragging = false;
            }
            self.toggle_todo_panel_compact();
            return Effect::Repaint;
        }
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
            // 1. 悬停：只在指向的概览标记或可展开段落变化时重绘
            MouseEventKind::Moved => {
                let hover = if layout.in_body(mouse.row) && layout.in_rail(mouse.column) {
                    let marks = rail_marks(state.document.anchors.len(), height);
                    mark_at_row(&marks, body_row).map(|mark| mark.anchor)
                } else {
                    None
                };
                let before = state.hovered_paragraph().map(|span| span.key);
                let before_button = session.bottom_button.is_some_and(|(start, end)| {
                    state.pointer_row == Some(height.saturating_sub(1))
                        && state
                            .pointer_col
                            .is_some_and(|col| col >= start && col < end)
                });
                let in_body = layout.in_body(mouse.row) && mouse.column < layout.content_width;
                state.pointer_row = in_body.then_some(body_row);
                state.pointer_col = in_body.then_some(mouse.column);
                let after = state.hovered_paragraph().map(|span| span.key);
                let after_button = session.bottom_button.is_some_and(|(start, end)| {
                    state.pointer_row == Some(height.saturating_sub(1))
                        && state
                            .pointer_col
                            .is_some_and(|col| col >= start && col < end)
                });
                let changed =
                    hover != state.hover || before != after || before_button != after_button;
                state.hover = hover;
                changed
            }
            MouseEventKind::Drag(MouseButton::Left) if state.dragging => {
                let target = scroll_for_track_row(state, body_row, height);
                state.scroll_to(target, height)
            }
            // 2. 正文拖动：扩展选区，拖到上下边缘时顺带滚动
            MouseEventKind::Drag(MouseButton::Left) => match state.press {
                Some(anchor) => {
                    let mut scrolled = false;
                    if mouse.row <= layout.body_top {
                        scrolled = state.scroll_by(-1, height);
                    } else if usize::from(mouse.row - layout.body_top) + 1 >= height {
                        scrolled = state.scroll_by(1, height);
                    }
                    let row = usize::from(mouse.row.saturating_sub(layout.body_top))
                        .min(height.saturating_sub(1));
                    let head = selection::TextPoint {
                        row: state.scroll + row,
                        col: usize::from(mouse.column).min(usize::from(layout.content_width)),
                    };
                    let next = Some(selection::Selection { anchor, head });
                    let changed = next != state.selection;
                    state.selection = next;
                    changed || scrolled
                }
                None => false,
            },
            // 3. 松开：有选区则复制，否则按点击展开或收起段落
            MouseEventKind::Up(MouseButton::Left) => {
                state.dragging = false;
                let Some(press) = state.press.take() else {
                    return Effect::Nothing;
                };
                match state.selection.filter(|selection| !selection.is_empty()) {
                    Some(selection) => {
                        let lines = state
                            .document
                            .lines
                            .iter()
                            .map(|line| {
                                crate::render::activity_animation::strip_ansi_for_test(
                                    line.as_str(),
                                )
                            })
                            .collect::<Vec<_>>();
                        let text = selection::selected_text(
                            &lines,
                            &selection,
                            usize::from(layout.content_width),
                        );
                        if text.is_empty() {
                            state.selection = None;
                        } else {
                            state.copied = Some(text.chars().count());
                            session.pending_copy = Some(text);
                        }
                        true
                    }
                    None => {
                        state.selection = None;
                        match state.toggle_at(press.row) {
                            Some(anchor) => {
                                session.pending_toggle = Some(anchor);
                                true
                            }
                            None => false,
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left) => {
                // 新的按下清掉上一次选区与复制提示
                let cleared = state.selection.take().is_some() | state.copied.take().is_some();
                let handled = {
                    // 2. 标题：回到当前消息开头
                    if mouse.row < layout.body_top {
                        match state.current_anchor() {
                            Some(index) => state.jump_to_anchor(index, height),
                            None => false,
                        }
                    } else if layout.in_body(mouse.row)
                        && usize::from(mouse.row - layout.body_top) + 1 == height
                        && session
                            .bottom_button
                            .is_some_and(|(start, end)| mouse.column >= start && mouse.column < end)
                    {
                        // 输入框正上方的“回到底部”按钮：直接跳到最新输出，不开始拖选
                        state.scroll_to(usize::MAX, height)
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
                        // 5. 正文：先记下按下位置，松开时再区分点击与拖选
                        state.press = Some(selection::TextPoint {
                            row: state.scroll + body_row,
                            col: usize::from(mouse.column),
                        });
                        false
                    } else {
                        false
                    }
                };
                handled || cleared
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

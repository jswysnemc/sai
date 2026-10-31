//! 分界线拖动与侧栏关闭控件；优先于聊天选择和审查面板处理鼠标。

use super::input::Effect;
use super::sidebar_control::SidebarControl;
use super::sidebar_palette::{fg, SidebarPalette};
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEventKind};

/// 【终端侧栏】【分界反馈】悬停和拖动时仅提亮分界线，不改变左右视图背景。
/// 参数: control 为交互状态，rows 为绘制行；返回: 无。
pub(super) fn decorate_divider(control: &SidebarControl, rows: &mut [String]) {
    if !control.divider_hovered && !control.dragging {
        return;
    }
    let palette = SidebarPalette::current();
    let before = format!("{}│", fg(palette.border));
    let after = format!("\x1b[7m{}│\x1b[27m", fg(palette.accent));
    for row in rows {
        *row = row.replacen(&before, &after, 1);
    }
}

impl ReplRuntime {
    /// 【终端侧栏】【尺寸交互】拖动分界线时持续消费鼠标事件，避免误选正文或移动输入光标。
    /// 参数: event 为终端事件；返回: 处理效果，不属于控件时为空。
    pub(super) fn sidebar_control_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        let Event::Mouse(mouse) = event else {
            if matches!(event, Event::Resize(..) | Event::FocusLost) {
                self.sidebar_control.dragging = false;
                self.sidebar_control.divider_hovered = false;
                self.sidebar_control.close_hovered = false;
            }
            return None;
        };
        let side = layout.sidebar?;
        let in_height = mouse.row < layout.rows;
        let divider = in_height && mouse.column == side.left;
        let close = layout.sidebar_close_at(mouse.column, mouse.row);
        let control = &mut self.sidebar_control;
        // 1. 【终端侧栏】【拖动捕获】按下之后允许指针离开分界线，以当前位置计算宽度
        if control.dragging
            && matches!(
                mouse.kind,
                MouseEventKind::Drag(MouseButton::Left) | MouseEventKind::Up(MouseButton::Left)
            )
        {
            let width = layout.cols.saturating_sub(mouse.column);
            if layout.review {
                control.review_width = Some(width);
            } else {
                control.width = Some(width);
            }
            // 2. 【终端侧栏】【宽度夹紧】存储实际可用宽度，终端缩窄时不覆盖已有偏好
            let geometry = control.geometry(layout.cols, layout.rows, layout.review)?;
            if layout.review {
                control.review_width = Some(geometry.width);
            } else {
                control.width = Some(geometry.width);
            }
            if mouse.kind == MouseEventKind::Up(MouseButton::Left) {
                control.dragging = false;
                control.divider_hovered = mouse.column == geometry.left && in_height;
            }
            return Some(Effect::Repaint);
        }
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) && (divider || close) {
            session.state.press = None;
            session.state.selection = None;
            session.state.dragging = false;
            session.state.pointer_row = None;
            session.state.pointer_col = None;
            session.sidebar.hovered = None;
            session.sidebar.preview.hovered = None;
            session.sidebar.preview.selection = Default::default();
            if close {
                control.close();
                session.sidebar.focused = false;
                session.sidebar.preview.close();
                return Some(Effect::Refresh);
            }
            control.dragging = true;
            control.divider_hovered = true;
            control.close_hovered = false;
            return Some(Effect::Repaint);
        }
        if mouse.kind == MouseEventKind::Moved {
            let changed = control.divider_hovered != divider || control.close_hovered != close;
            control.divider_hovered = divider;
            control.close_hovered = close;
            if divider || close {
                session.sidebar.hovered = None;
                session.sidebar.preview.hovered = None;
                session.state.pointer_row = None;
                session.state.pointer_col = None;
                session.state.hover = None;
                session.state.header_hover = false;
                return Some(if changed {
                    Effect::Repaint
                } else {
                    Effect::Nothing
                });
            }
            // 3. 【终端侧栏】【悬停离开】让其他区域继续处理同一事件，重绘由输入分流统一触发
        }
        None
    }
}

//! 分界线拖动；优先于聊天选择和审查面板处理鼠标。

#[cfg(test)]
#[path = "sidebar_resize_tests.rs"]
mod tests;

use super::input::Effect;
use super::sidebar_control::SidebarControl;
use super::sidebar_palette::{fg, SidebarPalette};
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEventKind};

/// 【终端侧栏】【分界反馈】整条分界使用相同细线，悬停与拖动只改变颜色
/// 参数：control 为交互，rows 为绘制行，column 为分界列，row_offset 为首行坐标，height 为总高度
/// 返回：无，只修改分界字符，不改变聊天或面板背景
pub(super) fn decorate_divider(
    control: &SidebarControl,
    rows: &mut [String],
    column: usize,
    _row_offset: usize,
    _height: usize,
) {
    let palette = SidebarPalette::current();
    let active = control.divider_hovered || control.dragging;
    for row in rows.iter_mut() {
        let color = if active {
            palette.accent
        } else {
            palette.border
        };
        *row = replace_divider(row, column, &format!("\x1b[0m{}│\x1b[0m", fg(color)));
    }
}

/// 【终端侧栏】【分界定位】跳过 ANSI 控制序列，按显示列定位分界字符，保留其他文字与样式
/// 参数：text 为整行，column 为目标显示列，replacement 为新字符与样式；返回：更新后的行
fn replace_divider(text: &str, column: usize, replacement: &str) -> String {
    let mut index = 0;
    let mut visible = 0;
    while index < text.len() {
        if text.as_bytes()[index] == 0x1b {
            index = crate::render::terminal_image::escape_sequence_end(text, index).max(index + 1);
            continue;
        }
        let ch = text[index..].chars().next().unwrap();
        if visible == column && ch == '│' {
            return format!(
                "{}{replacement}{}",
                &text[..index],
                &text[index + ch.len_utf8()..]
            );
        }
        visible += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        index += ch.len_utf8();
        if visible > column {
            break;
        }
    }
    text.to_string()
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
            }
            return None;
        };
        let side = layout.sidebar?;
        let in_height = mouse.row < layout.rows;
        let divider = in_height && mouse.column == side.left;
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
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) && divider {
            session.state.press = None;
            session.state.selection = None;
            session.state.dragging = false;
            session.state.pointer_row = None;
            session.state.pointer_col = None;
            session.sidebar.hovered = None;
            session.sidebar.preview.hovered = None;
            session.sidebar.preview.selection = Default::default();
            control.dragging = true;
            control.divider_hovered = true;
            control.scrollbar = Default::default();
            return Some(Effect::Repaint);
        }
        if mouse.kind == MouseEventKind::Moved {
            let changed = control.divider_hovered != divider;
            control.divider_hovered = divider;
            if divider {
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

//! 鼠标光标请求与编辑草稿之间的桥接，保持附件占位块原子性。

use super::composer_frame::CompletionPanelKind;
use super::ReplRuntime;
use crate::cli::repl_clipboard::ReplClipboardState;
use crossterm::event::{Event, MouseButton, MouseEventKind};

/// 【终端】【补全鼠标】斜杠命令与引用候选面板上的鼠标交互结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli) enum CompletionMouseEffect {
    Ignored,
    Consumed,
    Redraw,
    Select(usize),
    Accept(CompletionPanelKind, usize, bool),
}

impl ReplRuntime {
    /// 【终端】【点击定位】提取最新单击位置，附件占位块内部的点击吸附到最近边界。
    /// 参数: input 为编辑草稿，clipboard 为附件状态；返回: 字符偏移，无有效请求时为空。
    pub(in crate::cli) fn take_composer_mouse_cursor(
        &mut self,
        input: &str,
        clipboard: &ReplClipboardState,
    ) -> Option<usize> {
        let cursor = self.composer_selection.take_cursor(input)?;
        Some(snap_cursor(input, clipboard, cursor))
    }

    /// 【终端】【流式点击】将单击位置直接应用到运行期间草稿，不复制完整输入或附件。
    /// 参数: 无；返回: 是否需要重绘输入光标。
    pub(super) fn apply_stream_mouse_cursor(&mut self) -> bool {
        let draft = &mut self.stream_draft;
        let Some(cursor) = self.composer_selection.take_cursor(&draft.text) else {
            return false;
        };
        draft.cursor = snap_cursor(&draft.text, &draft.clipboard, cursor);
        draft.slash_selection = 0;
        true
    }

    /// 【终端】【补全鼠标】处理斜杠/引用候选面板的悬停、滚轮滚动与左键点选补全。
    ///
    /// 参数:
    /// - `event`: 原始终端事件
    /// - `input`: 当前输入草稿文本
    /// - `cursor`: 光标字符偏移
    /// - `current_selection`: 当前选中候选序号
    ///
    /// 返回:
    /// - 鼠标事件是否被补全面板消费及期望动作
    pub(in crate::cli) fn handle_completion_mouse(
        &mut self,
        event: &Event,
        input: &str,
        cursor: usize,
        current_selection: usize,
    ) -> CompletionMouseEffect {
        let Event::Mouse(mouse) = event else {
            return CompletionMouseEffect::Ignored;
        };
        if self.background_panel_active()
            || self.image_viewer_active()
            || self.composer_panels_dismissed()
            || self.queue_panel_active()
            || self.agent_panel_active()
        {
            self.completion_clicks.clear();
            self.completion_pointer = None;
            return CompletionMouseEffect::Ignored;
        }
        let Some(signature) = self.last_composer_signature.as_ref() else {
            return CompletionMouseEffect::Ignored;
        };
        let inside = signature.in_completion_area(mouse.column, mouse.row);
        let hit = signature.completion_hit_at(mouse.column, mouse.row);

        // 1. 悬停追踪：移入或跨行移动时刷新高亮，离开面板后自动撤除悬停态
        if matches!(mouse.kind, MouseEventKind::Moved) {
            let active_hit = hit.filter(|entry| !entry.disabled);
            let prev_hit = self
                .completion_pointer
                .and_then(|(col, row)| signature.completion_hit_at(col, row))
                .filter(|entry| !entry.disabled);
            if active_hit.is_some() {
                self.completion_pointer = Some((mouse.column, mouse.row));
                return if prev_hit != active_hit {
                    CompletionMouseEffect::Redraw
                } else {
                    CompletionMouseEffect::Consumed
                };
            }
            if self.completion_pointer.take().is_some() {
                return CompletionMouseEffect::Redraw;
            }
            return CompletionMouseEffect::Ignored;
        }

        // 2. 滚轮浏览：指针在补全面板范围内滚动全部匹配命令/引用，不劫持外侧记录滚动
        if matches!(
            mouse.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            if !inside {
                return CompletionMouseEffect::Ignored;
            }
            let total = self.completion_count(input, cursor);
            if total == 0 {
                return CompletionMouseEffect::Consumed;
            }
            let next = if mouse.kind == MouseEventKind::ScrollUp {
                current_selection.saturating_sub(1)
            } else {
                (current_selection + 1).min(total - 1)
            };
            if next != current_selection {
                return CompletionMouseEffect::Select(next);
            }
            return CompletionMouseEffect::Consumed;
        }

        // 3. 左键点选：仅当按下与松开落在同一候选行时选中并应用补全
        let clicked = self.completion_clicks.update(*mouse, hit);
        if let Some(target) = clicked {
            self.composer_selection.clear();
            self.completion_pointer = None;
            return CompletionMouseEffect::Accept(target.kind, target.index, target.disabled);
        }
        if inside && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            self.composer_selection.clear();
            return CompletionMouseEffect::Consumed;
        }
        CompletionMouseEffect::Ignored
    }
}

/// 【终端】【附件边界】鼠标定位不进入附件占位块内部，使用字符索引避免中文字节偏移错误。
/// 参数: input 为草稿，clipboard 为附件状态，cursor 为点击偏移；返回: 合法字符偏移。
fn snap_cursor(input: &str, clipboard: &ReplClipboardState, mut cursor: usize) -> usize {
    for span in clipboard.block_spans(input) {
        if cursor > span.start && cursor < span.end {
            cursor = if cursor - span.start <= span.end - cursor {
                span.start
            } else {
                span.end
            };
            break;
        }
    }
    cursor.min(input.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【附件定位】长文本占位块内的点击吸附到边界，普通字符位置保持不变。
    /// 参数: 无；返回: 无。
    #[test]
    fn mouse_cursor_snaps_to_atomic_block_edges() {
        let mut clipboard = ReplClipboardState::default();
        let mut input = "中文 ".to_string();
        let mut cursor = input.chars().count();
        assert!(clipboard.paste_text_into_input(&mut input, &mut cursor, "long text ".repeat(300)));
        let span = clipboard.block_spans(&input)[0];
        assert_eq!(snap_cursor(&input, &clipboard, span.start + 1), span.start);
        assert_eq!(snap_cursor(&input, &clipboard, span.end - 1), span.end);
        assert_eq!(snap_cursor(&input, &clipboard, 1), 1);
    }
}

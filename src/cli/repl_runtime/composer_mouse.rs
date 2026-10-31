//! 鼠标光标请求与编辑草稿之间的桥接，保持附件占位块原子性。

use super::ReplRuntime;
use crate::cli::repl_clipboard::ReplClipboardState;

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

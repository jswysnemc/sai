use super::*;

impl ComposerFrame {
    /// 根据当前列数计算输入、补全和光标布局。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 当前宽度下的 composer 布局
    pub(super) fn layout(&self, cols: usize) -> ComposerLayout {
        let cols = cols.max(1);
        // 输入在圆角盒内折行：扣除左右边框与左侧模式 accent
        let content_cols = chrome_input_content_cols(cols);
        let lines = repl_input_lines(&self.input);
        let display_lines = if self.input.is_empty() {
            vec![placeholder_text()]
        } else {
            lines.clone()
        };
        let mut offset = 0;
        let mut styled_display_lines = display_lines
            .iter()
            .map(|line| {
                let styled = crate::cli::repl_input_render::style_clipboard_line(
                    line,
                    offset,
                    &self.clipboard_blocks,
                );
                offset += line.chars().count() + 1;
                styled
            })
            .collect::<Vec<_>>();
        if let Some(ghost) = bang_ghost_suffix(&self.input) {
            if let Some(first) = styled_display_lines.first_mut() {
                first.push_str(&format!("\x1b[2m{ghost}\x1b[0m"));
            }
        }
        // 1. 【终端】【输入视口】先折行再裁剪，光标和绘制共用视觉行坐标
        let (cursor_col, cursor_row) =
            repl_cursor_position_for_cols("", &self.input, self.cursor, content_cols);
        let visual_lines = styled_display_lines
            .iter()
            .flat_map(|line| super::styled_line::wrap_styled_line(line, content_cols))
            .collect::<Vec<_>>();
        let (styled_display_lines, cursor_row_offset) = super::input_viewport::select_rows(
            &visual_lines,
            usize::from(cursor_row),
            usize::from(REPL_MAX_VISIBLE_INPUT_ROWS),
        );
        let input_rows = styled_display_lines.len() as u16;
        let mention_panel = if self.panels_dismissed {
            // 收起后重新输入前不再弹出，避免 Esc 看起来失灵
            MentionPanel::new(Vec::new(), 0)
        } else {
            MentionPanel::new(self.mention_candidates.clone(), self.slash_selection)
        };
        let slash_panel = if self.panels_dismissed
            || mention_panel.is_visible()
            || self.cursor != self.input.chars().count()
        {
            SlashPanel::new("", 0, self.streaming)
        } else {
            SlashPanel::new(&self.input, self.slash_selection, self.streaming)
        };
        let shell_hint =
            ShellHintPanel::new(&self.input, &self.chrome.model, &self.chrome.directory);
        ComposerLayout {
            styled_display_lines,
            input_rows,
            slash_panel,
            mention_panel,
            shell_hint,
            cursor_col,
            cursor_row_offset,
        }
    }
}

/// 返回空输入框的灰色提示文本。
///
/// 返回:
/// - 包含快捷操作说明的 ANSI 文本
fn placeholder_text() -> String {
    // 静态提示，避免空输入时底栏因轮询闪烁
    format!("\x1b[2m{}\x1b[0m", static_placeholder_tip())
}

/// 返回当前轮次的占位提示。
///
/// 首条是输入引导，之后每轮换一条功能提示，见 placeholder_tips。
fn static_placeholder_tip() -> &'static str {
    crate::cli::repl_runtime::placeholder_tips::current_tip()
}

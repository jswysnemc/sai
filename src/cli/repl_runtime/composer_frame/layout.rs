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
        let (display_lines, collapsed) = if self.input.is_empty() {
            (vec![placeholder_text()], false)
        } else {
            let visible =
                repl_visible_input_lines("", &lines, REPL_MAX_VISIBLE_INPUT_ROWS, self.is_pasted);
            (visible.lines, visible.collapsed)
        };
        let mut styled_display_lines =
            style_display_lines(&display_lines, &lines, collapsed, &self.clipboard_blocks);
        // 仅输入 `!` 时附上幽灵说明，光标仍停在 `!` 后（不计入 ghost 宽度）
        let ghost = bang_ghost_suffix(&self.input);
        if let Some(ghost) = ghost.as_deref() {
            if let Some(first) = styled_display_lines.first_mut() {
                first.push_str(&format!("\x1b[2m{ghost}\x1b[0m"));
            }
        }
        // 行数必须按带 ghost 的正文算：ghost 会让首行折行变高，
        // 用不含 ghost 的正文算出的行数偏小，底部状态行会被挤出保留区
        let mut measured_lines = display_lines.clone();
        if let Some(ghost) = ghost.as_deref() {
            if let Some(first) = measured_lines.first_mut() {
                first.push_str(ghost);
            }
        }
        let input_rows = repl_prompt_rows_for_cols("", &measured_lines, content_cols).max(1);
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
        // 折叠判定用显式标志：原文恰好 3 行时显示行数与原始行数相等，
        // 按长度比较会走错分支，把光标画到 composer 边框之外
        let (cursor_col, cursor_row_offset) = if !collapsed {
            repl_cursor_position_for_cols("", &self.input, self.cursor, content_cols)
        } else {
            let last_line = display_lines.last().map(String::as_str).unwrap_or_default();
            // 用与绘制同一个折行模拟器求末行落点，而不是对宽度取模：
            // 末行在列边界处是宽字符（CJK / emoji）时，宽字符会整体移到下一行，
            // 取模算出的列会落在错误位置，光标被画到 composer 边框之外
            let (end_col, _) = repl_cursor_position_for_cols(
                "",
                last_line,
                last_line.chars().count(),
                content_cols,
            );
            (end_col, input_rows.saturating_sub(1))
        };
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

/// 按原始输入行起点给显示行应用剪贴板块颜色。
fn style_display_lines(
    display_lines: &[String],
    raw_lines: &[String],
    collapsed: bool,
    spans: &[ReplClipboardBlockSpan],
) -> Vec<String> {
    let mut offsets = Vec::with_capacity(raw_lines.len());
    let mut offset = 0usize;
    for (index, line) in raw_lines.iter().enumerate() {
        offsets.push(offset);
        offset += line.chars().count() + usize::from(index + 1 < raw_lines.len());
    }
    if !collapsed {
        return display_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                crate::cli::repl_input_render::style_clipboard_line(line, offsets[index], spans)
            })
            .collect();
    }
    display_lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            if index > 0 && index + 1 < display_lines.len() {
                return line.clone();
            }
            let raw_index = if index + 1 == display_lines.len() {
                raw_lines.len().saturating_sub(1)
            } else {
                index
            };
            let line_start = offsets.get(raw_index).copied().unwrap_or_default();
            crate::cli::repl_input_render::style_clipboard_line(line, line_start, spans)
        })
        .collect()
}

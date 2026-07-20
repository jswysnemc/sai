use super::slash_panel::SlashPanel;
use super::viewport::InlineViewport;
use crate::cli::repl_chrome::{chrome_fixed_rows, chrome_rule, ReplChrome};
use crate::cli::repl_clipboard::ReplClipboardBlockSpan;
use crate::cli::repl_input_render::{
    repl_cursor_position_for_cols, repl_line_rows_for_cols, repl_prompt_rows_for_cols,
    repl_visible_input_lines,
};
use crate::cli::repl_text::{repl_input_lines, visible_width};
use crate::cli::REPL_MAX_VISIBLE_INPUT_ROWS;
use anyhow::Result;
use crossterm::cursor::{MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::Write;

/// 可从输入 source 按当前终端宽度重绘的 REPL composer。
#[derive(Clone)]
pub(super) struct ComposerFrame {
    chrome: ReplChrome,
    input: String,
    cursor: usize,
    is_pasted: bool,
    clipboard_blocks: Vec<ReplClipboardBlockSpan>,
    slash_selection: usize,
}

impl ComposerFrame {
    /// 创建当前输入状态的 composer source。
    ///
    /// 参数:
    /// - `chrome`: 底栏状态
    /// - `input`: 原始输入文本
    /// - `cursor`: 光标字符偏移
    /// - `is_pasted`: 是否为粘贴内容
    /// - `clipboard_blocks`: 剪贴板原子块区间
    /// - `slash_selection`: slash 命令面板的当前选中项
    ///
    /// 返回:
    /// - 可重绘的 composer source
    pub(super) fn new(
        chrome: ReplChrome,
        input: String,
        cursor: usize,
        is_pasted: bool,
        clipboard_blocks: Vec<ReplClipboardBlockSpan>,
        slash_selection: usize,
    ) -> Self {
        Self {
            chrome,
            input,
            cursor,
            is_pasted,
            clipboard_blocks,
            slash_selection,
        }
    }

    /// 返回 composer 在指定终端宽度下的视觉行数。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - composer 所需视觉行数
    pub(super) fn height(&self, cols: usize) -> u16 {
        let layout = self.layout(cols);
        if layout.slash_panel.is_visible() {
            return 2u16
                .saturating_add(layout.input_rows)
                .saturating_add(layout.slash_panel.height());
        }
        chrome_fixed_rows() + layout.input_rows
    }

    /// 将 composer 写入 viewport 底部并恢复输入光标位置。
    ///
    /// 参数:
    /// - `output`: 终端输出句柄
    /// - `viewport`: 当前历史与 composer 分区
    ///
    /// 返回:
    /// - 绘制是否成功
    pub(super) fn draw<W: Write>(&self, output: &mut W, viewport: &InlineViewport) -> Result<()> {
        let cols = usize::from(viewport.size().cols);
        let top = viewport.composer_top();
        let height = viewport.composer_height();
        let layout = self.layout(cols);

        // 1. 先清理整个保留区域，避免输入行数或补全提示缩短后残留旧内容
        for row_offset in 0..height {
            queue!(
                output,
                MoveTo(0, top.saturating_add(row_offset)),
                Clear(ClearType::CurrentLine)
            )?;
        }

        let mut row = top;
        // 2. 顶线、输入正文、底线和状态栏均从 source 按当前宽度重新计算
        queue!(output, MoveTo(0, row), Print(chrome_rule(cols)))?;
        row = row.saturating_add(1);

        let input_start_row = row;
        for line in &layout.styled_display_lines {
            queue!(output, MoveTo(0, row), Print(line))?;
            row = row.saturating_add(repl_line_rows_for_cols("", line, cols).max(1));
        }

        queue!(output, MoveTo(0, row), Print(chrome_rule(cols)))?;
        row = row.saturating_add(1);
        let end_row = if layout.slash_panel.is_visible() {
            layout.slash_panel.draw(output, row, cols)?;
            row.saturating_add(layout.slash_panel.height())
        } else {
            queue!(output, MoveTo(0, row), Print(self.chrome.footer_line(cols)))?;
            row.saturating_add(1)
        };

        // 3. composer 是受管区域底部：面板收起或行数减少后下方残留一并清除；
        //    贴底时无下方区域，跳过以免 MoveTo 越界被 clamp 到底行误清 footer
        if end_row < viewport.size().rows {
            queue!(output, MoveTo(0, end_row), Clear(ClearType::FromCursorDown))?;
        }

        // 4. 历史插入会移动终端光标，最后必须把它放回可继续编辑的位置
        queue!(
            output,
            MoveTo(
                layout.cursor_col,
                input_start_row.saturating_add(layout.cursor_row_offset)
            ),
            Show
        )?;
        output.flush()?;
        Ok(())
    }

    /// 根据当前列数计算输入、补全和光标布局。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 当前宽度下的 composer 布局
    fn layout(&self, cols: usize) -> ComposerLayout {
        let cols = cols.max(1);
        let lines = repl_input_lines(&self.input);
        let display_lines = if self.input.is_empty() {
            vec![placeholder_text()]
        } else {
            repl_visible_input_lines("", &lines, REPL_MAX_VISIBLE_INPUT_ROWS, self.is_pasted)
        };
        let styled_display_lines =
            style_display_lines(&display_lines, &lines, &self.clipboard_blocks);
        let input_rows = repl_prompt_rows_for_cols("", &display_lines, cols).max(1);
        let slash_panel = SlashPanel::new(&self.input, self.slash_selection);
        let (cursor_col, cursor_row_offset) = if display_lines.len() == lines.len() {
            repl_cursor_position_for_cols("", &self.input, self.cursor, cols)
        } else {
            let last_line = display_lines.last().map(String::as_str).unwrap_or_default();
            (
                (visible_width(last_line) % cols).min(u16::MAX as usize) as u16,
                input_rows.saturating_sub(1),
            )
        };
        ComposerLayout {
            styled_display_lines,
            input_rows,
            slash_panel,
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
    // 1. 每次启动种子不同，并按墙钟轮询下一条小技巧
    let text = super::super::composer_tips::current_composer_tip();
    format!("\x1b[2m{text}\x1b[0m")
}

/// composer 在单一终端宽度下的计算结果。
struct ComposerLayout {
    styled_display_lines: Vec<String>,
    input_rows: u16,
    slash_panel: SlashPanel,
    cursor_col: u16,
    cursor_row_offset: u16,
}

/// 按原始输入行起点给显示行应用剪贴板块颜色。
fn style_display_lines(
    display_lines: &[String],
    raw_lines: &[String],
    spans: &[ReplClipboardBlockSpan],
) -> Vec<String> {
    let mut offsets = Vec::with_capacity(raw_lines.len());
    let mut offset = 0usize;
    for (index, line) in raw_lines.iter().enumerate() {
        offsets.push(offset);
        offset += line.chars().count() + usize::from(index + 1 < raw_lines.len());
    }
    if display_lines.len() == raw_lines.len() {
        return display_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                super::super::repl_input_render::style_clipboard_line(line, offsets[index], spans)
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
            super::super::repl_input_render::style_clipboard_line(line, line_start, spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::ComposerFrame;
    use crate::agent::AgentMode;
    use crate::cli::repl_chrome::ReplChrome;
    use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};

    /// 验证 composer 在固定 viewport 内写入底部，并将光标放回输入行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn draws_at_viewport_bottom_and_restores_input_cursor() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let frame = ComposerFrame::new(chrome, "hello".to_string(), 5, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 40, rows: 12 }, frame.height(40), 8);
        let mut output = Vec::new();

        frame.draw(&mut output, &viewport).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[9;1H"));
        assert!(output.contains("\x1b[10;6H"));
    }

    /// 验证 slash 命令面板隐藏常规状态栏并展示命令说明。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn slash_panel_keeps_input_frame_visible_above_command_descriptions() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let frame = ComposerFrame::new(chrome, "/".to_string(), 1, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
        let mut output = Vec::new();

        frame.draw(&mut output, &viewport).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("/model"));
        assert!(output.matches('─').count() >= 2);
        assert!(output.contains("/"));
        assert!(!output.contains("120k"));
    }

    /// 验证空输入框显示灰色操作提示。
    #[test]
    fn empty_composer_shows_placeholder() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
        let mut output = Vec::new();

        frame.draw(&mut output, &viewport).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("/"));
        assert!(output.contains("!"));
        assert!(output.contains("\x1b[2m"));
    }

    /// 验证悬浮 composer 绘制后清除其下方残留内容。
    #[test]
    fn floating_composer_clears_stale_rows_below() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
        let mut output = Vec::new();

        frame.draw(&mut output, &viewport).unwrap();

        let output = String::from_utf8(output).unwrap();
        // composer 顶部在行 4（0 起），高 4 行，末行之后（行 8 → 1 起第 9 行）清到屏底
        assert!(output.contains("\x1b[9;1H\x1b[J"));
    }

    /// 验证贴底 composer 不发出越界清除，footer 行保持完整。
    #[test]
    fn bottom_pinned_composer_keeps_footer_row() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        // 历史充满屏幕：composer 固定在底部，末行即屏幕最后一行
        viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 60);
        let mut output = Vec::new();

        frame.draw(&mut output, &viewport).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("\x1b[J"), "贴底时不能清除 footer 行");
        assert!(output.contains("gpt"));
    }
}

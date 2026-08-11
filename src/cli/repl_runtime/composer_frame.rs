use super::shell_hint_panel::{bang_ghost_suffix, ShellHintPanel};
use super::slash_panel::SlashPanel;
use super::viewport::InlineViewport;
use crate::cli::repl_chrome::{
    chrome_fixed_rows, chrome_input_content_cols, chrome_panel_row, CHROME_ACCENT_COLS,
    CHROME_INPUT_PAD_ROWS, ReplChrome,
};
use crate::cli::repl_clipboard::ReplClipboardBlockSpan;
use crate::cli::repl_input_render::{
    repl_cursor_position_for_cols, repl_prompt_rows_for_cols,
    repl_visible_input_lines,
};
use crate::cli::repl_text::{repl_input_lines, visible_width};
use crate::cli::REPL_MAX_VISIBLE_INPUT_ROWS;
use crate::render::terminal_paint::paint_lock;
use anyhow::Result;
use crossterm::cursor::{MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::Write;

/// composer 一次绘制的完整内容签名。
///
/// 覆盖屏幕上会出现的每一处：位置、各区域文本与光标落点。
/// 两次签名相同即代表重绘不会改变任何像素。
#[derive(Clone, PartialEq, Eq)]
pub(super) struct ComposerSignature {
    top: u16,
    height: u16,
    cols: usize,
    panel_lines: Vec<String>,
    input_lines: Vec<String>,
    footer: Option<String>,
    slash_lines: Vec<String>,
    shell_lines: Vec<String>,
    cursor_col: u16,
    cursor_row: u16,
}

/// 可从输入 source 按当前终端宽度重绘的 REPL composer。
#[derive(Clone)]
pub(super) struct ComposerFrame {
    chrome: ReplChrome,
    input: String,
    cursor: usize,
    is_pasted: bool,
    clipboard_blocks: Vec<ReplClipboardBlockSpan>,
    slash_selection: usize,
    /// 输入框上方的沉底面板行（todo 快照 / 排队消息 / agent 提示）
    panel_lines: Vec<String>,
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
            panel_lines: Vec::new(),
        }
    }

    /// 设置输入框上方的沉底面板行。
    ///
    /// 参数:
    /// - `lines`: 已按当前宽度截断的 ANSI 面板行
    ///
    /// 返回:
    /// - 无
    pub(super) fn set_panel_lines(&mut self, lines: Vec<String>) {
        self.panel_lines = lines;
    }

    /// 返回当前 composer 绑定的 chrome 状态。
    ///
    /// 返回:
    /// - chrome 引用
    pub(super) fn chrome(&self) -> &ReplChrome {
        &self.chrome
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
        let panel_rows = self.panel_lines.len().min(usize::from(u16::MAX)) as u16;
        let pads = CHROME_INPUT_PAD_ROWS.saturating_mul(2);
        // slash / shell 提示时收起状态行，仍保留输入上下空白间距
        if layout.slash_panel.is_visible() {
            return panel_rows
                .saturating_add(pads)
                .saturating_add(layout.input_rows)
                .saturating_add(layout.slash_panel.height());
        }
        if layout.shell_hint.is_visible() {
            return panel_rows
                .saturating_add(pads)
                .saturating_add(layout.input_rows)
                .saturating_add(layout.shell_hint.height());
        }
        panel_rows
            .saturating_add(chrome_fixed_rows())
            .saturating_add(layout.input_rows)
    }

    /// 绘制 composer，并在内容与上次完全一致时跳过重绘。
    ///
    /// composer 每 32ms 重绘一次，而绘制是"逐行清除再打印"。
    /// Windows Terminal 不合并这两步，清与画之间的空窗表现为底部闪烁。
    /// 内容未变时直接复用上次结果，闪烁随之消失。
    ///
    /// 参数:
    /// - `output`: 终端输出
    /// - `viewport`: 当前历史与 composer 分区
    /// - `previous`: 上次绘制的内容签名
    ///
    /// 返回:
    /// - 光标最终所在行与本次内容签名
    pub(super) fn draw_lines<W: Write>(
        &self,
        output: &mut W,
        viewport: &InlineViewport,
        previous: Option<&ComposerSignature>,
    ) -> Result<(u16, ComposerSignature)> {
        let _paint = paint_lock();
        let cols = usize::from(viewport.size().cols);
        let top = viewport.composer_top();
        let height = viewport.composer_height();
        let layout = self.layout(cols);
        let content_cols = chrome_input_content_cols(cols);
        // 光标落在彩条右侧的内容区
        let drawn_cursor_col = layout
            .cursor_col
            .saturating_add(CHROME_ACCENT_COLS as u16)
            .min(cols.saturating_sub(1) as u16);
        let cursor_row = {
            let mut row = top;
            row = row.saturating_add(self.panel_lines.len().min(usize::from(u16::MAX)) as u16);
            // 输入正文上方有固定空白行，光标落在空白之下
            row.saturating_add(CHROME_INPUT_PAD_ROWS)
                .saturating_add(layout.cursor_row_offset)
        };
        let signature = ComposerSignature {
            top,
            height,
            cols,
            panel_lines: self.panel_lines.clone(),
            input_lines: layout.styled_display_lines.clone(),
            footer: if layout.slash_panel.is_visible() || layout.shell_hint.is_visible() {
                None
            } else {
                Some(self.chrome.footer_line(content_cols))
            },
            slash_lines: layout.slash_panel.rendered_lines(cols),
            shell_lines: layout.shell_hint.rendered_lines(cols),
            cursor_col: drawn_cursor_col,
            cursor_row,
        };
        // 与上次完全一致：连光标位置都没动，重绘只会带来闪烁
        if previous == Some(&signature) {
            return Ok((cursor_row, signature));
        }

        // 1. 先清理整个保留区域，避免输入行数或补全提示缩短后残留旧内容
        for row_offset in 0..height {
            queue!(
                output,
                MoveTo(0, top.saturating_add(row_offset)),
                Clear(ClearType::CurrentLine)
            )?;
        }

        let mut row = top;
        // 2. 沉底面板（todo 快照 / 排队消息 / agent 提示）渲染在输入面板上方
        for line in &self.panel_lines {
            queue!(output, MoveTo(0, row), Print(line))?;
            row = row.saturating_add(1);
        }
        // 3. 细引导线 + 灰底面板：输入上下各留一行空白，再接状态行
        let mode = self.chrome.mode;
        let panel_body_top = row;
        for _ in 0..CHROME_INPUT_PAD_ROWS {
            queue!(
                output,
                MoveTo(0, row),
                Print(chrome_panel_row(mode, "", cols))
            )?;
            row = row.saturating_add(1);
        }
        let input_start_row = row;
        for line in &layout.styled_display_lines {
            for segment in wrap_styled_line(line, content_cols) {
                queue!(
                    output,
                    MoveTo(0, row),
                    Print(chrome_panel_row(mode, &segment, cols))
                )?;
                row = row.saturating_add(1);
            }
        }
        for _ in 0..CHROME_INPUT_PAD_ROWS {
            queue!(
                output,
                MoveTo(0, row),
                Print(chrome_panel_row(mode, "", cols))
            )?;
            row = row.saturating_add(1);
        }

        let end_row = if layout.slash_panel.is_visible() {
            layout.slash_panel.draw(output, row, cols)?;
            row.saturating_add(layout.slash_panel.height())
        } else if layout.shell_hint.is_visible() {
            layout.shell_hint.draw(output, row, cols)?;
            row.saturating_add(layout.shell_hint.height())
        } else {
            // 输入与状态同一块面板底，中间不画分割线（对齐 OpenCode）
            let status = self.chrome.footer_line(content_cols);
            queue!(
                output,
                MoveTo(0, row),
                Print(chrome_panel_row(mode, &status, cols))
            )?;
            row.saturating_add(1)
        };


        // 4. composer 是受管区域底部：面板收起或行数减少后下方残留一并清除；
        //    贴底时无下方区域，跳过以免 MoveTo 越界被 clamp 到底行误清 footer
        if end_row < viewport.size().rows {
            queue!(output, MoveTo(0, end_row), Clear(ClearType::FromCursorDown))?;
        }

        // 5. 历史插入会移动终端光标，最后必须把它放回可继续编辑的位置
        let drawn_cursor_row = input_start_row.saturating_add(layout.cursor_row_offset);
        queue!(output, MoveTo(drawn_cursor_col, drawn_cursor_row), Show)?;
        output.flush()?;
        Ok((drawn_cursor_row, signature))
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
        // 输入在圆角盒内折行：扣除左右边框与左侧模式 accent
        let content_cols = chrome_input_content_cols(cols);
        let lines = repl_input_lines(&self.input);
        let (display_lines, collapsed) = if self.input.is_empty() {
            (vec![placeholder_text()], false)
        } else {
            let visible = repl_visible_input_lines(
                "",
                &lines,
                REPL_MAX_VISIBLE_INPUT_ROWS,
                self.is_pasted,
            );
            (visible.lines, visible.collapsed)
        };
        let mut styled_display_lines =
            style_display_lines(&display_lines, &lines, collapsed, &self.clipboard_blocks);
        // 仅输入 `!` 时附上幽灵说明，光标仍停在 `!` 后（不计入 ghost 宽度）
        if let Some(ghost) = bang_ghost_suffix(&self.input) {
            if let Some(first) = styled_display_lines.first_mut() {
                first.push_str(&format!("\x1b[2m{ghost}\x1b[0m"));
            }
        }
        let input_rows = repl_prompt_rows_for_cols("", &display_lines, content_cols).max(1);
        let slash_panel = SlashPanel::new(&self.input, self.slash_selection);
        let shell_hint = ShellHintPanel::new(
            &self.input,
            &self.chrome.model,
            &self.chrome.directory,
        );
        // 折叠判定用显式标志：原文恰好 3 行时显示行数与原始行数相等，
        // 按长度比较会走错分支，把光标画到 composer 边框之外
        let (cursor_col, cursor_row_offset) = if !collapsed {
            repl_cursor_position_for_cols("", &self.input, self.cursor, content_cols)
        } else {
            let last_line = display_lines.last().map(String::as_str).unwrap_or_default();
            (
                (visible_width(last_line) % content_cols).min(u16::MAX as usize) as u16,
                input_rows.saturating_sub(1),
            )
        };
        ComposerLayout {
            styled_display_lines,
            input_rows,
            slash_panel,
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
    // 1. 每次启动种子不同，并按墙钟轮询下一条小技巧
    let text = super::super::composer_tips::current_composer_tip();
    format!("\x1b[2m{text}\x1b[0m")
}

/// 按显示列宽切分含 ANSI 的输入行，供圆角盒逐行绘制。
fn wrap_styled_line(text: &str, cols: usize) -> Vec<String> {
    use unicode_width::UnicodeWidthChar;
    let cols = cols.max(1);
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut width = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            current.push_str(&text[index..end]);
            index = end.max(index + ch.len_utf8());
            continue;
        }
        let char_width = ch.width().unwrap_or(0);
        if char_width > 0 && width > 0 && width + char_width > cols {
            lines.push(std::mem::take(&mut current));
            width = 0;
        }
        current.push(ch);
        width = width.saturating_add(char_width);
        index += ch.len_utf8();
    }
    lines.push(current);
    lines
}

/// composer 在单一终端宽度下的计算结果。
struct ComposerLayout {
    styled_display_lines: Vec<String>,
    input_rows: u16,
    slash_panel: SlashPanel,
    shell_hint: ShellHintPanel,
    cursor_col: u16,
    cursor_row_offset: u16,
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

        frame.draw_lines(&mut output, &viewport, None).unwrap();

        let output = String::from_utf8(output).unwrap();
        // 最细引导线前景 + 面板灰底 + 输入文本
        assert!(output.contains("\x1b[38;5;208m"));
        assert!(output.contains('▏'));
        assert!(output.contains("\x1b[48;5;236m"));
        assert!(output.contains("hello"));
    }

    /// 验证内容未变时跳过重绘。
    ///
    /// composer 每 32ms 刷新一次，逐行清除再打印在 Windows Terminal 下
    /// 表现为底部闪烁；内容一致时必须完全不产生输出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn skips_repaint_when_nothing_changed() {
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

        let mut first = Vec::new();
        let (_, signature) = frame.draw_lines(&mut first, &viewport, None).unwrap();
        assert!(!first.is_empty(), "首次绘制必须实际输出");

        let mut second = Vec::new();
        frame
            .draw_lines(&mut second, &viewport, Some(&signature))
            .unwrap();

        assert!(second.is_empty(), "内容未变时不应产生任何终端输出");
    }

    /// 验证输入变化后仍会重绘。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn repaints_after_the_input_changes() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let first_frame =
            ComposerFrame::new(chrome.clone(), "hello".to_string(), 5, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        viewport.update(
            TerminalSize { cols: 40, rows: 12 },
            first_frame.height(40),
            8,
        );
        let mut sink = Vec::new();
        let (_, signature) = first_frame.draw_lines(&mut sink, &viewport, None).unwrap();

        let changed = ComposerFrame::new(chrome, "hello world".to_string(), 11, false, Vec::new(), 0);
        let mut output = Vec::new();
        changed
            .draw_lines(&mut output, &viewport, Some(&signature))
            .unwrap();

        assert!(!output.is_empty(), "输入变化后必须重绘");
    }

    /// 验证输入 `!` 时展示 shell 提示与幽灵说明，并隐藏常规底栏。
    #[test]
    fn bang_prefix_shows_shell_hint_instead_of_footer() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt-test".to_string(),
            thinking: "auto".to_string(),
            directory: "/tmp".to_string(),
        };
        let frame = ComposerFrame::new(chrome, "!".to_string(), 1, false, Vec::new(), 0);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
        let mut output = Vec::new();
        frame.draw_lines(&mut output, &viewport, None).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains('!'));
        assert!(
            output.contains("Run a command") || output.contains("运行命令"),
            "missing shell hint: {output}"
        );
        assert!(output.contains("gpt-test"));
        assert!(!output.contains("120k"));
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

        frame.draw_lines(&mut output, &viewport, None).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("/model"));
        // slash 展开时仍保留细引导线输入面板，但不画状态分隔线
        assert!(output.contains("\x1b[38;5;208m"));
        assert!(output.contains('▏'));
        assert!(output.contains("\x1b[48;5;236m"));
        assert!(output.contains("/"));
        assert!(!output.contains("120k"));
    }

    /// 验证沉底面板行渲染在输入框顶线上方并计入高度。
    #[test]
    fn panel_lines_render_above_chrome_and_extend_height() {
        let chrome = ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "gpt".to_string(),
            thinking: "auto".to_string(),
            directory: "/workspace".to_string(),
        };
        let mut frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
        let base_height = frame.height(72);
        frame.set_panel_lines(vec![
            "\x1b[2m计划\x1b[0m \x1b[2m1/3\x1b[0m".to_string(),
            "\x1b[1m\x1b[36m▶\x1b[0m \x1b[1m\x1b[36mcurrent\x1b[0m".to_string(),
        ]);
        assert_eq!(frame.height(72), base_height + 2);

        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
        let mut output = Vec::new();
        frame.draw_lines(&mut output, &viewport, None).unwrap();
        let output = String::from_utf8(output).unwrap();
        let panel_at = output.find("计划").unwrap();
        let rule_at = output.find('─').unwrap();
        assert!(panel_at < rule_at, "面板行必须渲染在顶线之前");
    }

    /// 验证空输入框显示灰色轮询提示。
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

        frame.draw_lines(&mut output, &viewport, None).unwrap();

        let output = String::from_utf8(output).unwrap();
        let tip = crate::cli::composer_tips::current_composer_tip();
        // 轮询提示内容随种子与墙钟变化，只校验当前 tip 与 dim 样式
        assert!(!tip.is_empty());
        assert!(output.contains(tip));
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

        frame.draw_lines(&mut output, &viewport, None).unwrap();

        let output = String::from_utf8(output).unwrap();
        // 固定 2 行（分隔+状态）+ 1 行输入 = 3；顶部在行 4（0 起）时，末行之后为行 7 → 1 起第 8 行
        assert!(output.contains("\x1b[8;1H\x1b[J"));
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

        frame.draw_lines(&mut output, &viewport, None).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("\x1b[J"), "贴底时不能清除 footer 行");
        assert!(output.contains("gpt"));
    }
}

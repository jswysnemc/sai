use crate::i18n::text as t;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::Print;
use crossterm::{execute, queue};
use std::io::{self, Write};
use std::time::Duration;

/// 面板最大内容宽度（不含边框）。
const MAX_PANEL_WIDTH: usize = 84;
/// 面板最小内容宽度。
const MIN_PANEL_WIDTH: usize = 30;

/// 在主屏中央显示一个浮动面板，任意关闭键退出。
///
/// 面板绘制在现有内容之上；调用方在返回后负责全量重绘恢复底层内容。
///
/// 参数:
/// - `title`: 面板标题
/// - `body`: 面板正文（支持 ANSI 与多行）
///
/// 返回:
/// - 展示是否成功
pub(super) fn show_center_panel(title: &str, body: &str) -> Result<()> {
    let was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        crossterm::terminal::enable_raw_mode()?;
    }
    execute!(io::stdout(), Hide)?;
    let result = run_panel_loop(title, body);
    execute!(io::stdout(), Show)?;
    if !was_raw {
        crossterm::terminal::disable_raw_mode()?;
    }
    result
}

/// 面板绘制与按键循环。
///
/// 参数:
/// - `title`: 面板标题
/// - `body`: 面板正文
///
/// 返回:
/// - 循环是否成功结束
fn run_panel_loop(title: &str, body: &str) -> Result<()> {
    let mut offset = 0usize;
    loop {
        // 1. 每轮按当前终端尺寸重排并绘制（resize 后自动重新居中）
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let layout = PanelLayout::compute(cols, rows, title, body);
        offset = offset.min(layout.max_offset());
        layout.draw(&mut io::stdout(), offset)?;
        // 2. 阻塞等待按键；超时轮询以响应 resize
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(())
                }
                KeyCode::Up => offset = offset.saturating_sub(1),
                KeyCode::Down => offset = offset.saturating_add(1),
                KeyCode::PageUp => offset = offset.saturating_sub(10),
                KeyCode::PageDown => offset = offset.saturating_add(10),
                KeyCode::Home => offset = 0,
                KeyCode::End => offset = usize::MAX,
                _ => {}
            },
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

/// 一次面板绘制的布局参数。
struct PanelLayout {
    /// 面板左上角列
    origin_col: u16,
    /// 面板左上角行
    origin_row: u16,
    /// 内容区宽度（不含边框与内边距）
    content_width: usize,
    /// 内容区可视行数
    content_height: usize,
    /// 标题文本
    title: String,
    /// 预折行的正文
    lines: Vec<AnsiLine>,
}

impl PanelLayout {
    /// 按终端尺寸计算面板布局。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `rows`: 终端行数
    /// - `title`: 面板标题
    /// - `body`: 面板正文
    ///
    /// 返回:
    /// - 布局参数
    fn compute(cols: u16, rows: u16, title: &str, body: &str) -> Self {
        let cols = usize::from(cols.max(20));
        let rows = usize::from(rows.max(8));
        let content_width = cols
            .saturating_sub(8)
            .min(MAX_PANEL_WIDTH)
            .max(MIN_PANEL_WIDTH.min(cols.saturating_sub(4)));
        let lines = AnsiLine::wrap_block(body.trim_end_matches('\n'), content_width);
        // 上下边框各 1 行，外加 1 行留白余量
        let content_height = lines.len().min(rows.saturating_sub(4)).max(1);
        let panel_width = content_width + 4;
        let panel_height = content_height + 2;
        let origin_col = cols.saturating_sub(panel_width) / 2;
        let origin_row = rows.saturating_sub(panel_height) / 2;
        Self {
            origin_col: origin_col.min(u16::MAX as usize) as u16,
            origin_row: origin_row.min(u16::MAX as usize) as u16,
            content_width,
            content_height,
            title: title.to_string(),
            lines,
        }
    }

    /// 返回滚动偏移上限。
    ///
    /// 返回:
    /// - 最大偏移
    fn max_offset(&self) -> usize {
        self.lines.len().saturating_sub(self.content_height)
    }

    /// 将面板绘制到终端。
    ///
    /// 参数:
    /// - `output`: 终端输出句柄
    /// - `offset`: 内容滚动偏移
    ///
    /// 返回:
    /// - 绘制是否成功
    fn draw<W: Write>(&self, output: &mut W, offset: usize) -> Result<()> {
        let inner = self.content_width + 2;
        // 1. 顶边框嵌标题
        let top = frame_line('╭', '╮', &format!(" {} ", self.title), inner);
        queue!(
            output,
            MoveTo(self.origin_col, self.origin_row),
            Print(format!("\x1b[2m{top}\x1b[0m"))
        )?;
        // 2. 内容行：左右边框 + 定宽内容
        for row in 0..self.content_height {
            let content = self
                .lines
                .get(offset + row)
                .map(AnsiLine::as_str)
                .unwrap_or("");
            let padding = self
                .content_width
                .saturating_sub(visible_width_ansi(content));
            queue!(
                output,
                MoveTo(self.origin_col, self.origin_row + 1 + row as u16),
                Print(format!(
                    "\x1b[2m│\x1b[0m {content}{} \x1b[2m│\x1b[0m",
                    " ".repeat(padding)
                ))
            )?;
        }
        // 3. 底边框嵌操作提示（可滚动时附带位置）
        let hint = if self.max_offset() > 0 {
            format!(
                " {}/{} · ↑↓ {} · Esc {} ",
                offset + self.content_height,
                self.lines.len(),
                t("scroll", "滚动"),
                t("close", "关闭")
            )
        } else {
            format!(" Esc {} ", t("close", "关闭"))
        };
        let bottom = frame_line('╰', '╯', &hint, inner);
        queue!(
            output,
            MoveTo(
                self.origin_col,
                self.origin_row + 1 + self.content_height as u16
            ),
            Print(format!("\x1b[2m{bottom}\x1b[0m"))
        )?;
        output.flush()?;
        Ok(())
    }
}

/// 生成一条嵌入文字的边框线。
///
/// 参数:
/// - `left`: 左端角字符
/// - `right`: 右端角字符
/// - `label`: 嵌入文本
/// - `inner`: 边框内宽度
///
/// 返回:
/// - 边框行文本
fn frame_line(left: char, right: char, label: &str, inner: usize) -> String {
    let label_width = label.chars().map(char_width).sum::<usize>();
    let dashes = inner.saturating_sub(label_width);
    let head = dashes / 2;
    format!(
        "{left}{}{label}{}{right}",
        "─".repeat(head),
        "─".repeat(dashes - head)
    )
}

/// 计算含 ANSI 文本的显示宽度。
///
/// 参数:
/// - `text`: ANSI 文本
///
/// 返回:
/// - 显示列数
fn visible_width_ansi(text: &str) -> usize {
    let mut width = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        width += char_width(ch);
    }
    width
}

/// 计算单个字符显示宽度。
///
/// 参数:
/// - `ch`: 字符
///
/// 返回:
/// - 显示列数
fn char_width(ch: char) -> usize {
    unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_centers_panel_and_wraps_body() {
        let layout = PanelLayout::compute(100, 30, "帮助", &"行内容\n".repeat(5));
        assert!(layout.origin_col > 0);
        assert!(layout.origin_row > 0);
        assert_eq!(layout.content_height, 5);
        assert_eq!(layout.max_offset(), 0);
    }

    #[test]
    fn layout_scrolls_when_body_exceeds_height() {
        let body = (0..50).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
        let layout = PanelLayout::compute(80, 20, "t", &body);
        assert!(layout.max_offset() > 0);
    }

    #[test]
    fn frame_line_fits_inner_width() {
        let line = frame_line('╭', '╮', " 标题 ", 40);
        let width: usize = line.chars().map(char_width).sum();
        assert_eq!(width, 42);
    }

    #[test]
    fn draw_pads_content_to_fixed_width() {
        let layout = PanelLayout::compute(100, 30, "t", "short\n一段中文内容");
        let mut sink = Vec::new();
        layout.draw(&mut sink, 0).unwrap();
        let output = String::from_utf8(sink).unwrap();
        assert!(output.contains("short"));
        assert!(output.contains("一段中文内容"));
        assert!(output.contains('╭'));
        assert!(output.contains('╯'));
    }
}

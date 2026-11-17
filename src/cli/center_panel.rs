use crate::cli::mouse_menu::ClickTracker;
use crate::i18n::text as t;
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::panel_palette::{fg, PanelPalette};
use crate::render::readable_terminal_text;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::style::Print;
use crossterm::{execute, queue};
use std::io::{self, Write};
use std::time::Duration;

/// 面板最大内容宽度（不含边框）。
const MAX_PANEL_WIDTH: usize = 84;
/// 面板最小期望内容宽度（宽终端下）。
const MIN_PANEL_WIDTH: usize = 30;

/// 【居中面板】【入口】在当前屏幕中央显示浮动面板，支持键盘、滚轮与点击关闭。
///
/// 面板绘制在现有内容之上；调用方在返回后负责重绘恢复底层画面。
///
/// 参数:
/// - `title`: 面板标题
/// - `body`: 面板正文（支持 ANSI 与多行）
///
/// 返回:
/// - 展示是否成功
pub(super) fn show_center_panel(title: &str, body: &str) -> Result<()> {
    let _mouse = super::mouse_capture::MouseCapture::acquire()?;
    let was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        crossterm::terminal::enable_raw_mode()?;
    }
    struct PanelGuard {
        was_raw: bool,
    }
    impl Drop for PanelGuard {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), Show);
            if !self.was_raw {
                let _ = crossterm::terminal::disable_raw_mode();
            }
        }
    }
    let _guard = PanelGuard { was_raw };
    execute!(io::stdout(), Hide)?;
    run_panel_loop(title, body)
}

/// 【居中面板】【事件循环】驱动绘制、键盘翻页、滚轮滚动与点击关闭。
///
/// 参数:
/// - `title`: 面板标题
/// - `body`: 面板正文
///
/// 返回:
/// - 循环是否成功结束
fn run_panel_loop(title: &str, body: &str) -> Result<()> {
    let mut offset = 0usize;
    let mut click = ClickTracker::default();
    loop {
        // 1. 【居中面板】【自适应重排】每轮按实时终端尺寸重排并绘制，窗口缩放后自动重新居中
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        let layout = PanelLayout::compute(cols, rows, title, body);
        offset = offset.min(layout.max_offset());
        layout.draw(&mut io::stdout(), offset)?;
        // 2. 【居中面板】【事件监听】轮询键盘、鼠标滚轮、点击关闭与窗口缩放
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let page = layout.content_height.max(1);
        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                click.clear();
                match key.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(())
                    }
                    KeyCode::Up | KeyCode::Char('k') => offset = offset.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => offset = offset.saturating_add(1),
                    KeyCode::PageUp => offset = offset.saturating_sub(page),
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        offset = offset.saturating_add(page)
                    }
                    KeyCode::Home => offset = 0,
                    KeyCode::End => offset = usize::MAX,
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                let can_close = layout.is_close_hit(mouse.column, mouse.row);
                if click.update(mouse, can_close.then_some(())).is_some() {
                    return Ok(());
                }
                match mouse.kind {
                    MouseEventKind::ScrollUp => offset = offset.saturating_sub(3),
                    MouseEventKind::ScrollDown => offset = offset.saturating_add(3),
                    _ => {}
                }
            }
            Event::Resize(_, _) => click.clear(),
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
    /// 面板总列宽
    panel_width: usize,
    /// 面板总行数
    panel_height: usize,
    /// 标题文本
    title: String,
    /// 预折行的正文
    lines: Vec<AnsiLine>,
}

impl PanelLayout {
    /// 【居中面板】【布局计算】按真实终端尺寸计算居中位置与内容视口，窄屏严格不越界。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `rows`: 终端行数
    /// - `title`: 面板标题
    /// - `body`: 面板正文
    ///
    /// 返回:
    /// - 面板布局参数
    fn compute(cols: u16, rows: u16, title: &str, body: &str) -> Self {
        let cols = usize::from(cols.max(5));
        let rows = usize::from(rows.max(3));
        let max_inner = cols.saturating_sub(4).max(1);
        let content_width = cols
            .saturating_sub(8)
            .min(MAX_PANEL_WIDTH)
            .max(MIN_PANEL_WIDTH.min(max_inner))
            .min(max_inner)
            .max(1);
        let lines = AnsiLine::wrap_block(body.trim_end_matches('\n'), content_width);
        let max_body_rows = rows.saturating_sub(2).max(1);
        let preferred_rows = rows.saturating_sub(4).max(1).min(max_body_rows);
        let content_height = lines.len().max(1).min(preferred_rows);
        let panel_width = content_width + 4;
        let panel_height = content_height + 2;
        let origin_col = cols.saturating_sub(panel_width) / 2;
        let origin_row = rows.saturating_sub(panel_height) / 2;
        Self {
            origin_col: origin_col.min(u16::MAX as usize) as u16,
            origin_row: origin_row.min(u16::MAX as usize) as u16,
            content_width,
            content_height,
            panel_width,
            panel_height,
            title: title.to_string(),
            lines,
        }
    }

    /// 【居中面板】【滚动上限】返回正文滚动偏移的最大值。
    ///
    /// 参数:
    /// - `self`: 当前布局
    ///
    /// 返回:
    /// - 最大偏移行数
    fn max_offset(&self) -> usize {
        self.lines.len().saturating_sub(self.content_height)
    }

    /// 【居中面板】【关闭命中】点击面板底边框关闭提示或点击面板外部空白区均视为关闭动作。
    ///
    /// 参数:
    /// - `column`: 鼠标列号
    /// - `row`: 鼠标行号
    ///
    /// 返回:
    /// - 属于关闭区域时返回 true
    fn is_close_hit(&self, column: u16, row: u16) -> bool {
        let col = usize::from(column);
        let r = usize::from(row);
        let left = usize::from(self.origin_col);
        let top = usize::from(self.origin_row);
        let right = left + self.panel_width;
        let bottom = top + self.panel_height;
        if col < left || col >= right || r < top || r >= bottom {
            return true;
        }
        r + 1 == bottom
    }

    /// 【居中面板】【终端绘制】将标题框、内容行与底栏提示输出到终端。
    ///
    /// 参数:
    /// - `output`: 终端输出句柄
    /// - `offset`: 内容滚动偏移
    ///
    /// 返回:
    /// - 绘制是否成功
    fn draw<W: Write>(&self, output: &mut W, offset: usize) -> Result<()> {
        let palette = PanelPalette::current();
        let border = fg(palette.subtle);
        let inner = self.content_width + 2;
        // 1. 【居中面板】【顶边框】嵌入裁剪后的标题并对齐圆角细框
        let top = frame_line('╭', '╮', &format!(" {} ", self.title), inner);
        queue!(
            output,
            MoveTo(self.origin_col, self.origin_row),
            Print(format!("{border}{top}\x1b[0m"))
        )?;
        // 2. 【居中面板】【内容行】左右边框包裹定宽正文并进行明暗对比度校正
        for row in 0..self.content_height {
            let raw = self
                .lines
                .get(offset + row)
                .map(AnsiLine::as_str)
                .unwrap_or("");
            let clipped = clip_ansi_to_width(raw, self.content_width, "…");
            let content = readable_terminal_text(&clipped);
            let padding = self
                .content_width
                .saturating_sub(visible_width_ansi(&content));
            queue!(
                output,
                MoveTo(self.origin_col, self.origin_row + 1 + row as u16),
                Print(format!(
                    "{border}│\x1b[0m {content}\x1b[0m{} {border}│\x1b[0m",
                    " ".repeat(padding)
                ))
            )?;
        }
        // 3. 【居中面板】【底边框】嵌入自适应宽度的滚动与关闭提示
        let hint = if self.max_offset() > 0 {
            let full = format!(
                " {}/{} · ↑↓/jk {} · Space {} · Esc {} ",
                (offset + self.content_height).min(self.lines.len()),
                self.lines.len(),
                t("scroll", "滚动"),
                t("page", "翻页"),
                t("close", "关闭")
            );
            if visible_width_ansi(&full) <= inner {
                full
            } else {
                format!(
                    " {}/{} · ↑↓ {} · Esc {} ",
                    (offset + self.content_height).min(self.lines.len()),
                    self.lines.len(),
                    t("scroll", "滚动"),
                    t("close", "关闭")
                )
            }
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
            Print(format!("{border}{bottom}\x1b[0m"))
        )?;
        output.flush()?;
        Ok(())
    }
}

/// 【居中面板】【边框拼装】剥离 ANSI 计算标签可见宽度并在窄屏自动裁剪，严格保证不破框。
///
/// 参数:
/// - `left`: 左端圆角字符
/// - `right`: 右端圆角字符
/// - `label`: 嵌入标题或提示文本（可含 ANSI）
/// - `inner`: 边框内部总列宽
///
/// 返回:
/// - 可见宽度严格等于 `inner + 2` 的边框行文本
fn frame_line(left: char, right: char, label: &str, inner: usize) -> String {
    if inner == 0 {
        return format!("{left}{right}");
    }
    let clipped = if visible_width_ansi(label) > inner {
        clip_ansi_to_width(label, inner, "…")
    } else {
        label.to_string()
    };
    let label_width = visible_width_ansi(&clipped);
    let dashes = inner.saturating_sub(label_width);
    let head = dashes / 2;
    format!(
        "{left}{}{clipped}{}{right}",
        "─".repeat(head),
        "─".repeat(dashes - head)
    )
}

/// 【居中面板】【可见宽度】计算含 ANSI 转义序列文本的真实显示列数。
///
/// 参数:
/// - `text`: 含 ANSI 序列的文本
///
/// 返回:
/// - 终端可见列宽
fn visible_width_ansi(text: &str) -> usize {
    ansi_visible_width(text)
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
        let body = (0..50)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let layout = PanelLayout::compute(80, 20, "t", &body);
        assert!(layout.max_offset() > 0);
    }

    #[test]
    fn frame_line_strips_ansi_and_clips_narrow_labels_without_overflow() {
        let styled = "\x1b[1;36m 彩色标题 \x1b[0m";
        let line = frame_line('╭', '╮', styled, 40);
        assert_eq!(visible_width_ansi(&line), 42);

        let overlong = " 10/50 · ↑↓ 滚动 · Space 翻页 · Esc 关闭 ";
        let narrow = frame_line('╰', '╯', overlong, 14);
        assert_eq!(visible_width_ansi(&narrow), 16);
        assert!(narrow.starts_with('╰') && narrow.ends_with('╯'));
    }

    #[test]
    fn narrow_terminal_draw_keeps_every_row_within_bounds() {
        let layout = PanelLayout::compute(
            18,
            8,
            "非常长的面板标题不会撑破边框",
            &"一段很长的中文帮助正文需要折行显示\n".repeat(10),
        );
        assert!(usize::from(layout.origin_col) + layout.panel_width <= 18);
        assert!(usize::from(layout.origin_row) + layout.panel_height <= 8);
        let mut sink = Vec::new();
        layout.draw(&mut sink, 0).unwrap();
        let output = String::from_utf8(sink).unwrap();
        assert!(output.contains('╭'));
        assert!(output.contains('╯'));
    }
}

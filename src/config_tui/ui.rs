use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::input::read_key;
use super::layout::{full_frame, master_detail_widths, scroll_start};
use super::theme::{
    help_line, selection_marks, ACCENT, BOLD, BRAND, CORNER_BOTTOM_LEFT, CORNER_BOTTOM_RIGHT,
    CORNER_TOP_LEFT, CORNER_TOP_RIGHT, DIM, LINE_HORIZONTAL, LINE_VERTICAL, MUTED, RESET,
};

/// 绘制近全屏菜单（可选右侧说明栏）。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `options`: 左侧选项
/// - `selected`: 选中下标
/// - `status`: 底栏自定义状态；空则用默认快捷键说明
///
/// 返回:
/// - 绘制是否成功
pub(crate) fn draw_menu(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    selected: usize,
    status: &str,
) -> Result<()> {
    draw_menu_with_details(stdout, title, options, &[], selected, status, "")
}

/// 绘制近全屏主从菜单：左侧选项、右侧说明。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `options`: 左侧选项
/// - `details`: 与选项对齐的说明；空切片则单栏铺满
/// - `selected`: 选中下标
/// - `status`: 底栏状态
/// - `subtitle`: 标题下的摘要行（如当前激活配置）
///
/// 返回:
/// - 绘制是否成功
pub(crate) fn draw_menu_with_details(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    details: &[String],
    selected: usize,
    status: &str,
    subtitle: &str,
) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let frame = full_frame(cols, rows);

    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, frame.x, frame.y, frame.width, frame.height, title)?;

    let inner_x = frame.x.saturating_add(2);
    let inner_w = frame.width.saturating_sub(4);
    let mut body_y = frame.y.saturating_add(1);
    if !subtitle.is_empty() {
        queue!(
            stdout,
            MoveTo(inner_x, body_y),
            Print(format!(
                "{MUTED}{}{RESET}",
                truncate(subtitle, inner_w as usize)
            ))
        )?;
        body_y = body_y.saturating_add(1);
        // 副标题与内容间画一条弱化分隔线，标题区从此有边界
        draw_inner_divider(stdout, frame.x, body_y, frame.width)?;
        body_y = body_y.saturating_add(1);
    }

    // 内容区：底部留一行给帮助条
    let body_bottom = frame.y.saturating_add(frame.height.saturating_sub(2));
    let body_h = body_bottom.saturating_sub(body_y).max(1);
    let (left_w, right_w) = if details.is_empty() {
        (inner_w, 0)
    } else {
        master_detail_widths(inner_w)
    };

    let visible_rows = body_h.max(1) as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        let row_y = body_y.saturating_add(row as u16);
        queue!(stdout, MoveTo(inner_x, row_y))?;
        if index >= options.len() {
            queue!(stdout, Print(" ".repeat(left_w as usize)))?;
            continue;
        }
        let (bar, style) = selection_marks(index == selected);
        let label = pad(&options[index], left_w.saturating_sub(2) as usize);
        queue!(stdout, Print(format!("{bar}{style} {label}{RESET}")))?;
    }
    draw_scroll_indicator(
        stdout,
        inner_x.saturating_add(left_w),
        body_y,
        body_h,
        options.len(),
        start,
        visible_rows,
    )?;

    if right_w > 0 {
        let detail_x = inner_x.saturating_add(left_w).saturating_add(2);
        let detail = details.get(selected).map(String::as_str).unwrap_or("");
        draw_wrapped_detail(stdout, detail_x, body_y, right_w, body_h, detail)?;
    }

    draw_status_bar(stdout, &frame, &menu_help(status))?;
    stdout.flush()?;
    Ok(())
}

/// 组装菜单底部帮助条。
fn menu_help(status: &str) -> String {
    if status.is_empty() {
        help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("open", "打开")),
            ("q", t("back", "返回")),
        ])
    } else {
        status.to_string()
    }
}

/// 在底部边框上内嵌绘制帮助 / 状态条。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `frame`: 外框矩形
/// - `content`: 已带样式的状态文本
///
/// 返回:
/// - 绘制是否成功
pub(crate) fn draw_status_bar(
    stdout: &mut io::Stdout,
    frame: &super::layout::FrameRect,
    content: &str,
) -> Result<()> {
    queue!(
        stdout,
        MoveTo(
            frame.x.saturating_add(2),
            frame.y.saturating_add(frame.height.saturating_sub(1))
        ),
        Print(format!(
            " {} ",
            truncate_ansi(content, frame.width.saturating_sub(6) as usize)
        ))
    )?;
    Ok(())
}

/// 在框内绘制一条弱化横向分隔线（两端与边框相接）。
fn draw_inner_divider(stdout: &mut io::Stdout, x: u16, y: u16, width: u16) -> Result<()> {
    let inner = width.saturating_sub(2) as usize;
    queue!(
        stdout,
        MoveTo(x, y),
        Print(format!(
            "{DIM}├{}┤{RESET}",
            LINE_HORIZONTAL.to_string().repeat(inner)
        ))
    )?;
    Ok(())
}

/// 列表右缘绘制滚动位置指示。
///
/// 列表完全可见时不画；超出时以细轨 + 滑块标出当前窗口位置，
/// 长列表里不再"不知道下面还有多少"。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `x`: 指示条所在列
/// - `y`: 列表首行
/// - `height`: 列表可见行数
/// - `total`: 列表总项数
/// - `start`: 当前窗口首项下标
/// - `visible`: 可见行数
///
/// 返回:
/// - 绘制是否成功
pub(super) fn draw_scroll_indicator(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    height: u16,
    total: usize,
    start: usize,
    visible: usize,
) -> Result<()> {
    if total <= visible || height == 0 {
        return Ok(());
    }
    let track = height as usize;
    let thumb_len = (track * visible / total).max(1);
    let max_start = total.saturating_sub(visible);
    let thumb_top = if max_start == 0 {
        0
    } else {
        (track.saturating_sub(thumb_len)) * start.min(max_start) / max_start
    };
    for row in 0..track {
        let glyph = if row >= thumb_top && row < thumb_top + thumb_len {
            format!("{MUTED}▐{RESET}")
        } else {
            format!("{DIM}▏{RESET}")
        };
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16)),
            Print(glyph)
        )?;
    }
    Ok(())
}

/// 在右侧栏绘制自动换行的说明文本。
pub(super) fn draw_wrapped_detail(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    text: &str,
) -> Result<()> {
    if width == 0 || height == 0 {
        return Ok(());
    }
    queue!(
        stdout,
        MoveTo(x, y),
        Print(format!(
            "{BRAND}{BOLD}◈ {}{RESET}",
            truncate(t("Details", "说明"), width.saturating_sub(2) as usize)
        ))
    )?;
    let lines = wrap_text(text, width as usize);
    let max_lines = height.saturating_sub(2) as usize;
    for (row, line) in lines.into_iter().take(max_lines).enumerate() {
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16).saturating_add(2)),
            Print(format!("{MUTED}{}{RESET}", pad(&line, width as usize)))
        )?;
    }
    Ok(())
}

/// 按显示宽度将文本折成多行。
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 || text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            current_w = 0;
            continue;
        }
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_w + w > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_w = 0;
        }
        current.push(ch);
        current_w += w;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub(crate) fn draw_box(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
) -> Result<()> {
    let inner = width.saturating_sub(2) as usize;
    let horizontal = LINE_HORIZONTAL.to_string().repeat(inner);
    queue!(
        stdout,
        MoveTo(x, y),
        Print(format!(
            "{DIM}{CORNER_TOP_LEFT}{horizontal}{CORNER_TOP_RIGHT}{RESET}"
        ))
    )?;
    for row in 1..height.saturating_sub(1) {
        queue!(
            stdout,
            MoveTo(x, y + row),
            Print(format!(
                "{DIM}{LINE_VERTICAL}{RESET}{}{DIM}{LINE_VERTICAL}{RESET}",
                " ".repeat(inner)
            ))
        )?;
    }
    queue!(
        stdout,
        MoveTo(x, y + height.saturating_sub(1)),
        Print(format!(
            "{DIM}{CORNER_BOTTOM_LEFT}{horizontal}{CORNER_BOTTOM_RIGHT}{RESET}"
        ))
    )?;
    // 标题嵌在顶边框上：品牌色菱形锚点 + 粗体标题，两侧留白与边框断开
    queue!(
        stdout,
        MoveTo(x + 2, y),
        Print(format!(
            " {BRAND}◆{RESET} {ACCENT}{BOLD}{}{RESET} ",
            title.trim()
        ))
    )?;
    Ok(())
}

/// 绘制供应商浏览器等场景的列：聚焦列亮标题，选中项用箭头而非反色。
pub(crate) fn draw_column(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
    items: &[String],
    selected: usize,
    active: bool,
) -> Result<()> {
    queue!(stdout, MoveTo(x, y))?;
    let title_style = if active {
        format!("{ACCENT}{BOLD}")
    } else {
        MUTED.to_string()
    };
    let marker = if active { "▸ " } else { "  " };
    queue!(
        stdout,
        Print(format!(
            "{title_style}{}{RESET}",
            pad(
                &truncate(&format!("{marker}{}", title.trim()), width as usize),
                width as usize
            )
        ))
    )?;
    let visible_rows = height.saturating_sub(2) as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        queue!(stdout, MoveTo(x, y + row as u16 + 1))?;
        if index >= items.len() {
            queue!(stdout, Print(" ".repeat(width as usize)))?;
            continue;
        }
        let (bar, style) = selection_marks(index == selected && active);
        let line = truncate(&items[index], width.saturating_sub(2) as usize);
        let style = if index == selected && !active {
            // 非聚焦列的选中项用次级高亮，避免与聚焦列抢视线
            super::theme::ACCENT
        } else {
            style
        };
        let bar = if index == selected && !active {
            format!("{ACCENT}·{RESET}")
        } else {
            bar
        };
        queue!(
            stdout,
            Print(format!(
                "{bar}{style} {}{RESET}",
                pad(&line, width.saturating_sub(2) as usize)
            ))
        )?;
    }
    Ok(())
}

/// 未保存更改时的退出选择。
pub(crate) enum UnsavedExitChoice {
    Save,
    Discard,
    Cancel,
}

/// 用配置界面的菜单询问如何处理未保存更改。
///
/// 参数:
/// - `stdout`: 终端输出
///
/// 返回:
/// - 保存、放弃或取消
pub(crate) fn confirm_unsaved_exit(stdout: &mut io::Stdout) -> Result<UnsavedExitChoice> {
    let mut selected = 0usize;
    loop {
        let options = [
            t("Save and exit", "保存并退出"),
            t("Discard changes", "放弃更改"),
            t("Cancel", "取消"),
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        draw_menu(
            stdout,
            t(" Unsaved changes ", " 未保存的更改 "),
            &options,
            selected,
            &help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("confirm", "确认")),
                ("Esc", t("back", "返回")),
            ]),
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(UnsavedExitChoice::Cancel),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => {
                return Ok(match selected {
                    0 => UnsavedExitChoice::Save,
                    1 => UnsavedExitChoice::Discard,
                    _ => UnsavedExitChoice::Cancel,
                });
            }
            _ => {}
        }
    }
}

pub(crate) fn message(stdout: &mut io::Stdout, text: &str) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let frame = full_frame(cols, rows);
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(
        stdout,
        frame.x,
        frame.y,
        frame.width,
        frame.height,
        t(" Notice ", " 提示 "),
    )?;
    let lines = wrap_text(text, frame.width.saturating_sub(4) as usize);
    for (row, line) in lines
        .into_iter()
        .take(frame.height.saturating_sub(4) as usize)
        .enumerate()
    {
        queue!(
            stdout,
            MoveTo(
                frame.x.saturating_add(2),
                frame.y.saturating_add(2).saturating_add(row as u16)
            ),
            Print(line)
        )?;
    }
    draw_status_bar(
        stdout,
        &frame,
        &help_line(&[(t("any key", "任意键"), t("continue", "继续"))]),
    )?;
    stdout.flush()?;
    let _ = read_key()?;
    Ok(())
}

/// 按显示宽度截断文本，超长时追加省略号。
pub(crate) fn truncate(value: &str, max: usize) -> String {
    if display_width(value) <= max {
        return value.to_string();
    }
    let mut width = 0usize;
    let mut output = String::new();
    let ellipsis_width = 1usize;
    for ch in value.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + char_width + ellipsis_width > max {
            break;
        }
        output.push(ch);
        width += char_width;
    }
    output.push('…');
    output
}

/// 按可见宽度截断带 ANSI 样式的文本（不足时原样返回）。
fn truncate_ansi(value: &str, max: usize) -> String {
    let mut width = 0usize;
    let mut output = String::new();
    let mut chars = value.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(value, index);
            output.push_str(&value[index..end]);
            // 跳过序列内剩余字符
            while chars.peek().is_some_and(|(next, _)| *next < end) {
                chars.next();
            }
            continue;
        }
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + char_width > max {
            break;
        }
        output.push(ch);
        width += char_width;
    }
    output.push_str(RESET);
    output
}

/// 计算文本在终端中的显示宽度。
pub(crate) fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub(crate) fn pad(value: &str, width: usize) -> String {
    let value = truncate(value, width);
    let len = display_width(&value);
    if len >= width {
        value
    } else {
        format!("{value}{}", " ".repeat(width - len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_width_matches_unicode_width() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("供应商"), 6);
        assert_eq!(display_width("a供b"), 4);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn truncate_respects_display_width() {
        assert_eq!(truncate("abcdef", 10), "abcdef");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("供应商配置", 6), "供应…");
    }

    #[test]
    fn pad_fills_to_display_width() {
        assert_eq!(display_width(&pad("供应商", 10)), 10);
        assert_eq!(display_width(&pad("abc", 5)), 5);
    }

    #[test]
    fn wrap_text_breaks_on_width_and_newlines() {
        assert_eq!(wrap_text("abcdef", 3), vec!["abc", "def"]);
        assert_eq!(wrap_text("ab\ncd", 10), vec!["ab", "cd"]);
    }

    /// 带 ANSI 的截断只统计可见宽度，样式序列完整保留。
    #[test]
    fn truncate_ansi_counts_visible_width_only() {
        let styled = format!("{ACCENT}abc{RESET}def");
        let output = truncate_ansi(&styled, 4);
        assert!(output.contains("abc"));
        assert!(output.contains('d'));
        assert!(!output.contains("ef"));
        assert!(output.starts_with(ACCENT));
    }
}

use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::input::read_key;
use super::layout::{full_frame, master_detail_widths, scroll_start};
use super::theme::{
    selection_marks, ACCENT, BOLD, BRAND, CORNER_BOTTOM_LEFT, CORNER_BOTTOM_RIGHT, CORNER_TOP_LEFT,
    CORNER_TOP_RIGHT, LINE_HORIZONTAL, LINE_VERTICAL, MUTED, RESET,
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
    }

    // 内容区：顶/底各留一行给副标题与底栏
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
        queue!(stdout, Print(format!("{bar} {style}{label}{RESET}")))?;
    }

    if right_w > 0 {
        let detail_x = inner_x.saturating_add(left_w).saturating_add(2);
        let detail = details.get(selected).map(String::as_str).unwrap_or("");
        draw_wrapped_detail(stdout, detail_x, body_y, right_w, body_h, detail)?;
    }

    queue!(
        stdout,
        MoveTo(frame.x.saturating_add(2), frame.y.saturating_add(frame.height.saturating_sub(1))),
        Print(format!(
            " {MUTED}{}{RESET} ",
            truncate(menu_help(status), frame.width.saturating_sub(6) as usize)
        ))
    )?;
    stdout.flush()?;
    Ok(())
}

fn menu_help(status: &str) -> &str {
    if status.is_empty() {
        t(
            "↑/↓ move · Enter open · q back",
            "↑/↓ 移动 · Enter 打开 · q 返回",
        )
    } else {
        status
    }
}

/// 在右侧栏绘制自动换行的说明文本。
fn draw_wrapped_detail(
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
            "{BRAND}{BOLD}{}{RESET}",
            truncate(t("Details", "说明"), width as usize)
        ))
    )?;
    let lines = wrap_text(text, width as usize);
    let max_lines = height.saturating_sub(2) as usize;
    for (row, line) in lines.into_iter().take(max_lines).enumerate() {
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16).saturating_add(2)),
            Print(format!(
                "{MUTED}{}{RESET}",
                pad(&line, width as usize)
            ))
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
            "{MUTED}{CORNER_TOP_LEFT}{horizontal}{CORNER_TOP_RIGHT}{RESET}"
        ))
    )?;
    for row in 1..height.saturating_sub(1) {
        queue!(
            stdout,
            MoveTo(x, y + row),
            Print(format!(
                "{MUTED}{LINE_VERTICAL}{RESET}{}{MUTED}{LINE_VERTICAL}{RESET}",
                " ".repeat(inner)
            ))
        )?;
    }
    queue!(
        stdout,
        MoveTo(x, y + height.saturating_sub(1)),
        Print(format!(
            "{MUTED}{CORNER_BOTTOM_LEFT}{horizontal}{CORNER_BOTTOM_RIGHT}{RESET}"
        ))
    )?;
    // 标题嵌在顶边框上，两侧留一格空白与边框线断开
    queue!(
        stdout,
        MoveTo(x + 2, y),
        Print(format!(" {BRAND}{BOLD}{title}{RESET} "))
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
                "{bar} {style}{}{RESET}",
                pad(&line, width.saturating_sub(2) as usize)
            ))
        )?;
    }
    Ok(())
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
    for (row, line) in lines.into_iter().take(frame.height.saturating_sub(4) as usize).enumerate() {
        queue!(
            stdout,
            MoveTo(
                frame.x.saturating_add(2),
                frame.y.saturating_add(2).saturating_add(row as u16)
            ),
            Print(line)
        )?;
    }
    queue!(
        stdout,
        MoveTo(
            frame.x.saturating_add(2),
            frame.y.saturating_add(frame.height.saturating_sub(1))
        ),
        Print(format!(
            " {MUTED}{}{RESET} ",
            t("Press any key to continue", "按任意键继续")
        ))
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
}

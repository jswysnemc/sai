use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::input::read_key;
use super::layout::{panel_width, scroll_start};

pub(crate) fn draw_menu(
    stdout: &mut io::Stdout,
    title: &str,
    options: &[String],
    selected: usize,
    status: &str,
) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    // 1. 内容宽度与 pad/truncate 统一使用显示宽度口径
    let content_w = options
        .iter()
        .map(|option| display_width(option))
        .max()
        .unwrap_or(20)
        .max(display_width(title))
        .max(display_width(menu_help(status)))
        + 6;
    // 2. 先取期望宽度再钳制到终端可用宽度，避免窄终端溢出换行
    let width = panel_width(content_w as u16, 56, cols.saturating_sub(4));
    let height = (options.len() as u16 + 5)
        .min(rows.saturating_sub(2))
        .max(7);
    let x = cols.saturating_sub(width) / 2;
    let y = rows.saturating_sub(height) / 2;

    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, title)?;
    queue!(
        stdout,
        MoveTo(x + 2, y + height - 1),
        SetAttribute(Attribute::Dim),
        Print(truncate(
            menu_help(status),
            width.saturating_sub(4) as usize
        )),
        SetAttribute(Attribute::Reset)
    )?;
    // 3. 选项数超过可见高度时按选中项滚动，避免溢出项堆积在底行
    let visible_rows = height.saturating_sub(4).max(1) as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        if index >= options.len() {
            break;
        }
        queue!(stdout, MoveTo(x + 2, y + row as u16 + 2))?;
        if index == selected {
            queue!(
                stdout,
                SetAttribute(Attribute::Reverse),
                Print(pad(&options[index], width.saturating_sub(4) as usize)),
                SetAttribute(Attribute::Reset)
            )?;
        } else {
            queue!(
                stdout,
                Print(pad(&options[index], width.saturating_sub(4) as usize))
            )?;
        }
    }
    stdout.flush()?;
    Ok(())
}

fn menu_help(status: &str) -> &str {
    if status.is_empty() {
        t(
            "[j/k] move [Enter] select [q] back",
            "[j/k]移动 [Enter]选择 [q]返回",
        )
    } else {
        status
    }
}

pub(crate) fn draw_box(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
) -> Result<()> {
    queue!(
        stdout,
        MoveTo(x, y),
        Print(format!(
            "┌{}┐",
            "─".repeat(width.saturating_sub(2) as usize)
        ))
    )?;
    for row in 1..height.saturating_sub(1) {
        queue!(
            stdout,
            MoveTo(x, y + row),
            Print(format!(
                "│{}│",
                " ".repeat(width.saturating_sub(2) as usize)
            ))
        )?;
    }
    queue!(
        stdout,
        MoveTo(x, y + height.saturating_sub(1)),
        Print(format!(
            "└{}┘",
            "─".repeat(width.saturating_sub(2) as usize)
        ))
    )?;
    queue!(
        stdout,
        MoveTo(x + 2, y),
        SetAttribute(Attribute::Bold),
        Print(title),
        SetAttribute(Attribute::Reset)
    )?;
    Ok(())
}

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
    if active {
        queue!(
            stdout,
            SetAttribute(Attribute::Bold),
            SetForegroundColor(Color::White),
            SetBackgroundColor(Color::DarkBlue),
            Print(pad(&truncate(title, width as usize), width as usize)),
            ResetColor,
            SetAttribute(Attribute::Reset)
        )?;
    } else {
        queue!(
            stdout,
            SetAttribute(Attribute::Bold),
            Print(pad(&truncate(title, width as usize), width as usize)),
            SetAttribute(Attribute::Reset)
        )?;
    }
    let visible_rows = height.saturating_sub(2) as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        if index >= items.len() {
            break;
        }
        queue!(stdout, MoveTo(x, y + row as u16 + 1))?;
        let line = truncate(&items[index], width as usize);
        if index == selected {
            queue!(
                stdout,
                SetAttribute(Attribute::Reverse),
                Print(pad(&line, width as usize)),
                SetAttribute(Attribute::Reset)
            )?;
        } else {
            queue!(stdout, Print(pad(&line, width as usize)))?;
        }
    }
    Ok(())
}

pub(crate) fn message(stdout: &mut io::Stdout, text: &str) -> Result<()> {
    queue!(
        stdout,
        Clear(ClearType::All),
        MoveTo(0, 0),
        Print(text),
        MoveTo(0, 2),
        Print(t("Press any key to continue", "按任意键继续"))
    )?;
    stdout.flush()?;
    let _ = read_key()?;
    Ok(())
}

/// 按显示宽度截断文本，超长时追加省略号。
///
/// 参数:
/// - `value`: 原始文本
/// - `max`: 最大显示宽度
///
/// 返回:
/// - 截断后的文本
pub(crate) fn truncate(value: &str, max: usize) -> String {
    // 1. 显示宽度未超限时原样返回
    if display_width(value) <= max {
        return value.to_string();
    }
    // 2. 按字符显示宽度累加，为省略号预留一列
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
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 按 Unicode 东亚宽度规则计算的列数
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

    /// 验证 ASCII、中文和组合字符的显示宽度口径。
    #[test]
    fn display_width_matches_unicode_width() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("供应商"), 6);
        assert_eq!(display_width("a供b"), 4);
        assert_eq!(display_width(""), 0);
    }

    /// 验证按显示宽度截断中文文本并附加省略号。
    #[test]
    fn truncate_respects_display_width() {
        assert_eq!(truncate("abcdef", 10), "abcdef");
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("供应商配置", 6), "供应…");
    }

    /// 验证 pad 结果的显示宽度与目标宽度一致。
    #[test]
    fn pad_fills_to_display_width() {
        assert_eq!(display_width(&pad("供应商", 10)), 10);
        assert_eq!(display_width(&pad("abc", 5)), 5);
    }
}

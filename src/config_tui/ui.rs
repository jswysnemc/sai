mod dialogs;
mod menu;
pub(crate) use dialogs::{confirm_delete, confirm_unsaved_exit, message, UnsavedExitChoice};
pub(crate) use menu::{draw_menu_interactive, status_bar_hits, strip_ansi_plain, MenuHitbox};

use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::layout::{scroll_start, FrameRect};
use super::mouse::HitRect;
use super::theme::{
    help_separator, row_marks, ACCENT, BOLD, BRAND, CORNER_BOTTOM_LEFT, CORNER_BOTTOM_RIGHT,
    CORNER_TOP_LEFT, CORNER_TOP_RIGHT, DIM, LINE_HORIZONTAL, LINE_VERTICAL, MUTED, PILL_BG, RESET,
};

/// 开始一帧同步更新。支持的终端会等整帧画完再提交，清屏不再闪一下。
pub(crate) fn begin_synced_frame(stdout: &mut impl Write) -> Result<()> {
    queue!(
        stdout,
        Print(crate::render::readable_terminal_text(
            &("\u{1b}[?2026h").to_string()
        ))
    )?;
    Ok(())
}

/// 结束同步更新并一次性刷出本帧。
pub(crate) fn end_synced_frame(stdout: &mut impl Write) -> Result<()> {
    queue!(
        stdout,
        Print(crate::render::readable_terminal_text(
            &("\u{1b}[?2026l").to_string()
        ))
    )?;
    stdout.flush()?;
    Ok(())
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
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn draw_status_bar(
    stdout: &mut io::Stdout,
    frame: &super::layout::FrameRect,
    content: &str,
) -> Result<()> {
    draw_status_bar_interactive(stdout, frame, content, None)
}

/// 【配置界面】【底栏交互绘制】绘制底栏帮助条并高亮指针悬停的快捷键段。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `frame`: 面板外框矩形
/// - `content`: 已带样式的状态文本
/// - `hovered`: 当前鼠标悬停的可点分段矩形
///
/// 返回:
/// - 绘制结果
pub(crate) fn draw_status_bar_interactive(
    stdout: &mut io::Stdout,
    frame: &super::layout::FrameRect,
    content: &str,
    hovered: Option<HitRect>,
) -> Result<()> {
    let fitted = fit_status_bar(content, frame.width.saturating_sub(6) as usize);
    let highlighted = match hovered {
        Some(rect) if rect.y == frame.y.saturating_add(frame.height.saturating_sub(1)) => {
            highlight_status_segment(frame, &fitted, rect)
        }
        _ => fitted,
    };
    queue!(
        stdout,
        MoveTo(
            frame.x.saturating_add(2),
            frame.y.saturating_add(frame.height.saturating_sub(1))
        ),
        Print(crate::render::readable_terminal_text(
            &(format!(" {highlighted} ")).to_string()
        ))
    )?;
    Ok(())
}

/// 【配置界面】【底栏高亮】仅对命中悬停矩形的帮助分段套用胶囊强调色，保留邻近段样式。
fn highlight_status_segment(frame: &FrameRect, fitted: &str, target: HitRect) -> String {
    let separator = help_separator();
    let sep_w = ansi_width(&separator);
    let mut col = usize::from(frame.x.saturating_add(3));
    let mut out = String::with_capacity(fitted.len() + 32);
    for (idx, seg) in fitted.split(&separator).enumerate() {
        if idx > 0 {
            out.push_str(&separator);
        }
        let seg_w = ansi_width(seg);
        if col == usize::from(target.x) && seg_w == usize::from(target.width) {
            out.push_str(&format!(
                "{PILL_BG}{ACCENT}{BOLD}{}{RESET}",
                strip_ansi_plain(seg)
            ));
        } else {
            out.push_str(seg);
        }
        col = col.saturating_add(seg_w).saturating_add(sep_w);
    }
    out
}

/// 计算带 ANSI 样式文本的显示宽度。
///
/// `display_width` 走 UnicodeWidthStr，会把样式序列本身计入列数；
/// 这里按 `truncate_ansi` 的口径跳过转义序列，两者必须一致。
///
/// 参数:
/// - `value`: 可能带样式的文本
///
/// 返回:
/// - 终端实际占用的列数
pub(super) fn ansi_width(value: &str) -> usize {
    let mut width = 0usize;
    let mut index = 0usize;
    while index < value.len() {
        let ch = value[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            index = crate::render::terminal_image::escape_sequence_end(value, index);
            continue;
        }
        width += UnicodeWidthChar::width(ch).unwrap_or(0);
        index += ch.len_utf8();
    }
    width
}

/// 按优先级压缩状态条，保证末段（返回 / 退出键）不被截掉。
///
/// 窄终端上整条从左到右截断会把 `q 返回` 连同 `d 删除` 一起切掉：
/// 用户既看不到怎么退出，也看不到哪个键是破坏性的。
/// 因此放不下时从中间丢段，始终保留首段与末段。
///
/// 参数:
/// - `content`: 已带样式的状态文本
/// - `max`: 可用显示宽度
///
/// 返回:
/// - 压缩后的状态文本
pub(super) fn fit_status_bar(content: &str, max: usize) -> String {
    // 必须用 ANSI 感知的宽度：display_width 走 UnicodeWidthStr，
    // 会把样式序列本身计入列数，与 truncate_ansi 的度量口径不一致
    let measured = ansi_width(content);
    if measured <= max {
        return content.to_string();
    }
    let separator = super::theme::help_separator();
    let segments = content.split(&separator).collect::<Vec<_>>();
    // 不足三段时无从取舍，退化为普通截断
    if segments.len() < 3 {
        return truncate_ansi(content, max);
    }
    let last = segments.len() - 1;
    // 从末尾往前逐段丢掉中间键，尽量留下靠前的键（搜索、移动），末段返回键始终保留
    for keep in (1..last).rev() {
        let mut candidate = segments[..keep].join(&separator);
        candidate.push_str(&separator);
        candidate.push_str(segments[last]);
        if ansi_width(&candidate) <= max {
            return candidate;
        }
    }
    let first_and_last = format!("{}{separator}{}", segments[0], segments[last]);
    if ansi_width(&first_and_last) <= max {
        return first_and_last;
    }
    if ansi_width(segments[last]) <= max {
        return segments[last].to_string();
    }
    truncate_ansi(segments[last], max)
}

/// 在框内绘制一条弱化横向分隔线（两端与边框相接）。
pub(super) fn draw_inner_divider(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
) -> Result<()> {
    let inner = width.saturating_sub(2) as usize;
    queue!(
        stdout,
        MoveTo(x, y),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{DIM}├{}┤{RESET}",
                LINE_HORIZONTAL.to_string().repeat(inner)
            ))
            .to_string()
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
    let thumb_top = (track.saturating_sub(thumb_len) * start.min(max_start))
        .checked_div(max_start)
        .unwrap_or_default();
    for row in 0..track {
        let glyph = if row >= thumb_top && row < thumb_top + thumb_len {
            format!("{MUTED}▐{RESET}")
        } else {
            format!("{DIM}▏{RESET}")
        };
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16)),
            Print(crate::render::readable_terminal_text(&(glyph).to_string()))
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
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{BRAND}{BOLD}◈ {}{RESET}",
                truncate(t("Details", "说明"), width.saturating_sub(2) as usize)
            ))
            .to_string()
        ))
    )?;
    let lines = wrap_text(text, width as usize);
    let max_lines = height.saturating_sub(2) as usize;
    for (row, line) in lines.into_iter().take(max_lines).enumerate() {
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16).saturating_add(2)),
            Print(crate::render::readable_terminal_text(
                &(format!("{MUTED}{}{RESET}", pad(&line, width as usize))).to_string()
            ))
        )?;
    }
    Ok(())
}

/// 按显示宽度将文本折成多行。
pub(super) fn wrap_text(text: &str, width: usize) -> Vec<String> {
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
        Print(crate::render::readable_terminal_text(
            &(format!("{DIM}{CORNER_TOP_LEFT}{horizontal}{CORNER_TOP_RIGHT}{RESET}")).to_string()
        ))
    )?;
    for row in 1..height.saturating_sub(1) {
        queue!(
            stdout,
            MoveTo(x, y + row),
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{DIM}{LINE_VERTICAL}{RESET}{}{DIM}{LINE_VERTICAL}{RESET}",
                    " ".repeat(inner)
                ))
                .to_string()
            ))
        )?;
    }
    queue!(
        stdout,
        MoveTo(x, y + height.saturating_sub(1)),
        Print(crate::render::readable_terminal_text(
            &(format!("{DIM}{CORNER_BOTTOM_LEFT}{horizontal}{CORNER_BOTTOM_RIGHT}{RESET}"))
                .to_string()
        ))
    )?;
    // 标题嵌在顶边框上：品牌色菱形锚点 + 粗体标题，两侧留白与边框断开
    queue!(
        stdout,
        MoveTo(x + 2, y),
        Print(crate::render::readable_terminal_text(
            &(format!(" {BRAND}◆{RESET} {ACCENT}{BOLD}{}{RESET} ", title.trim())).to_string()
        ))
    )?;
    Ok(())
}

/// 绘制供应商浏览器等场景的列：聚焦列亮标题，选中项用箭头而非反色。
/// @param stdout 终端输出；area 列的矩形区域；title 标题；items 条目；selected 选中索引；active 是否聚焦
/// @returns 绘制结果
#[allow(dead_code)]
pub(crate) fn draw_column(
    stdout: &mut io::Stdout,
    area: FrameRect,
    title: &str,
    items: &[String],
    selected: usize,
    active: bool,
) -> Result<()> {
    draw_column_interactive(stdout, area, title, items, selected, None, false, active)
}

/// 【配置界面】【交互分栏】绘制单栏列表并高亮鼠标悬停行。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `area`: 栏矩形区域
/// - `title`: 栏标题
/// - `items`: 条目文本列表
/// - `selected`: 当前选中下标
/// - `hovered`: 鼠标悬停下标
/// - `active`: 该栏是否处于焦点
///
/// 返回:
/// - 绘制结果
pub(crate) fn draw_column_interactive(
    stdout: &mut io::Stdout,
    area: FrameRect,
    title: &str,
    items: &[String],
    selected: usize,
    hovered: Option<usize>,
    header_hovered: bool,
    active: bool,
) -> Result<()> {
    let FrameRect {
        x,
        y,
        width,
        height,
    } = area;
    queue!(stdout, MoveTo(x, y))?;
    let title_style = if active {
        format!("{ACCENT}{BOLD}")
    } else if header_hovered {
        format!("{PILL_BG}{ACCENT}{BOLD}")
    } else {
        MUTED.to_string()
    };
    let marker = if active { "▸ " } else { "  " };
    queue!(
        stdout,
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{title_style}{}{RESET}",
                pad(
                    &truncate(&format!("{marker}{}", title.trim()), width as usize),
                    width as usize
                )
            ))
            .to_string()
        ))
    )?;
    let visible_rows = height.saturating_sub(2) as usize;
    let start = scroll_start(selected, visible_rows);
    for row in 0..visible_rows {
        let index = start + row;
        queue!(stdout, MoveTo(x, y + row as u16 + 1))?;
        if index >= items.len() {
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(" ".repeat(width as usize)).to_string()
                ))
            )?;
            continue;
        }
        let is_selected = index == selected && active;
        let is_hovered = hovered == Some(index);
        let (bar, style) = if index == selected && !active {
            // 非聚焦列的选中项保留次级点状锚点，若叠加悬停则轻度提亮
            (
                format!("{ACCENT}·{RESET}"),
                if is_hovered {
                    row_marks(false, true).1
                } else {
                    super::theme::ACCENT
                },
            )
        } else {
            row_marks(is_selected, is_hovered)
        };
        let line = truncate(&items[index], width.saturating_sub(2) as usize);
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{bar}{style} {}{RESET}",
                    pad(&line, width.saturating_sub(2) as usize)
                ))
                .to_string()
            ))
        )?;
    }
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
pub(super) fn truncate_ansi(value: &str, max: usize) -> String {
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

    /// 放不下时优先丢中间段，末段的返回键必须留下来。
    #[test]
    fn status_bar_keeps_the_trailing_back_key_when_narrow() {
        let help = super::super::theme::help_line(&[
            ("\u{2191}\u{2193}", "move"),
            ("Enter", "open"),
            ("d", "delete"),
            ("q", "back"),
        ]);
        let full_width = ansi_width(&help);
        let narrowed = fit_status_bar(&help, 20);

        assert!(ansi_width(&narrowed) <= 20);
        assert!(narrowed.contains("back"), "返回键不能被截掉：{narrowed}");
        assert!(full_width > 20);
    }

    /// 宽度足够时不改写内容。
    #[test]
    fn status_bar_is_untouched_when_it_fits() {
        let help = super::super::theme::help_line(&[("q", "back")]);
        assert_eq!(fit_status_bar(&help, 80), help);
    }
}

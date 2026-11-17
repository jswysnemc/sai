//! 配置界面通用终端绘制原语：同步帧、圆角面板、状态栏高亮、滚动指示器与交互分栏。

mod dialogs;
mod menu;
#[cfg(test)]
mod tests;
mod text;

pub(crate) use dialogs::{confirm_delete, confirm_unsaved_exit, message, UnsavedExitChoice};
pub(crate) use menu::{draw_menu_interactive, status_bar_hits, strip_ansi_plain, MenuHitbox};
use text::{ansi_width, fit_status_bar, format_box_title, truncate_ansi, wrap_text};
pub(crate) use text::{display_width, pad, truncate};

use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io::{self, Write};

use super::layout::{scroll_start, FrameRect};
use super::mouse::HitRect;
use super::search::SEARCH_CURSOR_MARK;
use super::theme::{
    help_separator, row_marks, ThemePalette, BOLD, CORNER_BOTTOM_LEFT, CORNER_BOTTOM_RIGHT,
    CORNER_TOP_LEFT, CORNER_TOP_RIGHT, LINE_HORIZONTAL, LINE_VERTICAL, RESET,
};

/// 【配置界面】【同步帧开启】开启终端同步帧缓冲，避免整屏清屏产生视觉闪烁。
///
/// 参数:
/// - `stdout`: 终端输出流
///
/// 返回:
/// - 写出结果
pub(crate) fn begin_synced_frame(stdout: &mut impl Write) -> Result<()> {
    queue!(
        stdout,
        Print(crate::render::readable_terminal_text(
            &("\u{1b}[?2026h").to_string()
        ))
    )?;
    Ok(())
}

/// 【配置界面】【同步帧提交】结束同步更新并立即刷新底层输出缓冲区。
///
/// 参数:
/// - `stdout`: 终端输出流
///
/// 返回:
/// - 刷新结果
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

/// 【配置界面】【静态底栏】在面板底部边框内嵌绘制无悬停态的状态条。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn draw_status_bar(
    stdout: &mut io::Stdout,
    frame: &FrameRect,
    content: &str,
) -> Result<()> {
    draw_status_bar_interactive(stdout, frame, content, None)
}

/// 【配置界面】【底栏交互绘制】绘制底栏帮助条并高亮指针悬停的快捷键分段。
pub(crate) fn draw_status_bar_interactive(
    stdout: &mut io::Stdout,
    frame: &FrameRect,
    content: &str,
    hovered: Option<HitRect>,
) -> Result<()> {
    let _ = draw_status_bar_with_cursor(stdout, frame, content, hovered)?;
    Ok(())
}

/// 【配置界面】【带光标底栏绘制】绘制状态栏并解析搜索编辑态内嵌的真实硬件光标坐标。
pub(super) fn draw_status_bar_with_cursor(
    stdout: &mut io::Stdout,
    frame: &FrameRect,
    content: &str,
    hovered: Option<HitRect>,
) -> Result<Option<(u16, u16)>> {
    let fitted = fit_status_bar(content, frame.width.saturating_sub(6) as usize);
    let bar_y = frame.y.saturating_add(frame.height.saturating_sub(1));
    // 1. 【配置界面】【光标提取】从压缩后的底栏文本中测算真实光标列号并剥离控制标记
    let cursor = fitted.find(SEARCH_CURSOR_MARK).map(|byte_pos| {
        let col_offset = ansi_width(&fitted[..byte_pos]) as u16;
        let max_col = frame.x.saturating_add(frame.width.saturating_sub(2));
        (
            frame
                .x
                .saturating_add(3)
                .saturating_add(col_offset)
                .min(max_col),
            bar_y,
        )
    });
    let clean = fitted.replace(SEARCH_CURSOR_MARK, "");
    // 2. 【配置界面】【悬停胶囊】命中底栏快捷键段时应用主题胶囊强调色
    let highlighted = match hovered {
        Some(rect) if rect.y == bar_y => highlight_status_segment(frame, &clean, rect),
        _ => clean,
    };
    queue!(
        stdout,
        MoveTo(frame.x.saturating_add(2), bar_y),
        Print(crate::render::readable_terminal_text(
            &(format!(" {highlighted} ")).to_string()
        ))
    )?;
    Ok(cursor)
}

/// 【配置界面】【底栏高亮】仅对命中悬停矩形的帮助分段套用胶囊强调色，保留邻近段样式。
fn highlight_status_segment(frame: &FrameRect, fitted: &str, target: HitRect) -> String {
    let palette = ThemePalette::current();
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
                "{}{}{BOLD}{}{RESET}",
                palette.pill_bg,
                palette.accent,
                strip_ansi_plain(seg)
            ));
        } else {
            out.push_str(seg);
        }
        col = col.saturating_add(seg_w).saturating_add(sep_w);
    }
    out
}

/// 【配置界面】【内部分隔线】在圆角框内绘制两端衔接边框的弱化水平分隔线。
pub(super) fn draw_inner_divider(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
) -> Result<()> {
    let palette = ThemePalette::current();
    let inner = width.saturating_sub(2) as usize;
    queue!(
        stdout,
        MoveTo(x, y),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}├{}┤{RESET}",
                palette.dim,
                LINE_HORIZONTAL.to_string().repeat(inner)
            ))
            .to_string()
        ))
    )?;
    Ok(())
}

/// 【配置界面】【滚动条绘制】当条目总数超出可视高度时在列表右缘绘制滑块位置指示器。
pub(crate) fn draw_scroll_indicator(
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
    let palette = ThemePalette::current();
    let track = height as usize;
    let thumb_len = (track * visible / total).max(1);
    let max_start = total.saturating_sub(visible);
    let thumb_top = (track.saturating_sub(thumb_len) * start.min(max_start))
        .checked_div(max_start)
        .unwrap_or_default();
    for row in 0..track {
        let glyph = if row >= thumb_top && row < thumb_top + thumb_len {
            format!("{}▐{RESET}", palette.muted)
        } else {
            format!("{}▏{RESET}", palette.dim)
        };
        queue!(
            stdout,
            MoveTo(x, y.saturating_add(row as u16)),
            Print(crate::render::readable_terminal_text(&glyph))
        )?;
    }
    Ok(())
}

/// 【配置界面】【详情栏绘制】在主从布局右侧栏绘制自动折行的说明卡片。
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
    let palette = ThemePalette::current();
    queue!(
        stdout,
        MoveTo(x, y),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}{BOLD}◈ {}{RESET}",
                palette.brand,
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
                &(format!("{}{}{RESET}", palette.muted, pad(&line, width as usize))).to_string()
            ))
        )?;
    }
    Ok(())
}

/// 【配置界面】【圆角外框】绘制圆角边框并将安全截断后的标题嵌入顶部边线。
pub(crate) fn draw_box(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
) -> Result<()> {
    let palette = ThemePalette::current();
    let inner = width.saturating_sub(2) as usize;
    let horizontal = LINE_HORIZONTAL.to_string().repeat(inner);
    queue!(
        stdout,
        MoveTo(x, y),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}{CORNER_TOP_LEFT}{horizontal}{CORNER_TOP_RIGHT}{RESET}",
                palette.dim
            ))
            .to_string()
        ))
    )?;
    for row in 1..height.saturating_sub(1) {
        queue!(
            stdout,
            MoveTo(x, y + row),
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{}{LINE_VERTICAL}{RESET}{}{}{LINE_VERTICAL}{RESET}",
                    palette.dim,
                    " ".repeat(inner),
                    palette.dim
                ))
                .to_string()
            ))
        )?;
    }
    queue!(
        stdout,
        MoveTo(x, y + height.saturating_sub(1)),
        Print(crate::render::readable_terminal_text(
            &(format!(
                "{}{CORNER_BOTTOM_LEFT}{horizontal}{CORNER_BOTTOM_RIGHT}{RESET}",
                palette.dim
            ))
            .to_string()
        ))
    )?;
    if let Some(badge) = format_box_title(title, width) {
        queue!(
            stdout,
            MoveTo(x + 2, y),
            Print(crate::render::readable_terminal_text(&badge))
        )?;
    }
    Ok(())
}

/// 【配置界面】【静态分栏】绘制不含鼠标悬停态的单栏列表。
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

/// 【配置界面】【交互分栏】绘制支持鼠标悬停与右侧溢出滚动条的单栏列表。
#[allow(clippy::too_many_arguments)]
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
    let palette = ThemePalette::current();
    let FrameRect {
        x,
        y,
        width,
        height,
    } = area;
    queue!(stdout, MoveTo(x, y))?;
    let title_style = if active {
        format!("{}{BOLD}", palette.accent)
    } else if header_hovered {
        format!("{}{}{BOLD}", palette.pill_bg, palette.accent)
    } else {
        palette.muted.to_string()
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
    // 1. 【配置界面】【分栏滚动条】列表超出可视高度时在栏右缘让出 1 列绘制滚动滑块
    let show_scroll = items.len() > visible_rows && width >= 4;
    let row_w = if show_scroll {
        width.saturating_sub(1)
    } else {
        width
    };
    for row in 0..visible_rows {
        let index = start + row;
        queue!(stdout, MoveTo(x, y + row as u16 + 1))?;
        if index >= items.len() {
            queue!(
                stdout,
                Print(crate::render::readable_terminal_text(
                    &(" ".repeat(row_w as usize)).to_string()
                ))
            )?;
            continue;
        }
        let is_selected = index == selected && active;
        let is_hovered = hovered == Some(index);
        let (bar, style) = if index == selected && !active {
            let bg = if is_hovered { palette.hover_bg } else { "" };
            (
                format!("{bg}{}·{RESET}", palette.accent),
                if is_hovered {
                    row_marks(false, true).1
                } else {
                    palette.accent
                },
            )
        } else {
            row_marks(is_selected, is_hovered)
        };
        let line = truncate(&items[index], row_w.saturating_sub(2) as usize);
        queue!(
            stdout,
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{bar}{style} {}{RESET}",
                    pad(&line, row_w.saturating_sub(2) as usize)
                ))
                .to_string()
            ))
        )?;
    }
    if show_scroll {
        draw_scroll_indicator(
            stdout,
            x.saturating_add(row_w),
            y.saturating_add(1),
            visible_rows as u16,
            items.len(),
            start,
            visible_rows,
        )?;
    }
    Ok(())
}

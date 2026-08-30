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
            fit_status_bar(content, frame.width.saturating_sub(6) as usize)
        ))
    )?;
    Ok(())
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
fn ansi_width(value: &str) -> usize {
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
fn fit_status_bar(content: &str, max: usize) -> String {
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
    let last = segments[segments.len() - 1];
    let first = segments[0];
    // 末段优先：先只留首段 + 末段，仍放不下就只留末段
    for candidate in [
        format!("{first}{separator}{last}"),
        format!("{separator}{last}"),
        last.to_string(),
    ] {
        if ansi_width(&candidate) <= max {
            return candidate;
        }
    }
    truncate_ansi(last, max)
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

/// 破坏性删除操作的确认弹窗。
///
/// 默认停在「取消」：删除键往往与目标键相邻（如 s/d、a/d），
/// 误触后无法撤销，因此默认选项必须是安全项。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `target`: 待删除对象名称
/// - `warning`: 副标题中的后果说明（可为空）
///
/// 返回:
/// - 是否确认删除
pub(crate) fn confirm_delete(
    stdout: &mut io::Stdout,
    title: &str,
    target: &str,
    warning: &str,
) -> Result<bool> {
    let mut selected = 1usize;
    loop {
        let options = [
            format!("{} {target}", t("Delete", "删除")),
            t("Cancel", "取消").to_string(),
        ];
        draw_menu_with_details(
            stdout,
            title,
            &options,
            &[],
            selected,
            &help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("confirm", "确认")),
                ("Esc", t("cancel", "取消")),
            ]),
            warning,
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => return Ok(selected == 0),
            _ => {}
        }
    }
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
    let lines = wrap_text(text, frame.width.saturating_sub(4) as usize);
    let page = frame.height.saturating_sub(4) as usize;
    // 内容超出一屏时可滚动：Skills 详情、校验错误这类长文本原先只取头部，
    // 直接断在句子中间且无法看到后面
    let max_offset = lines.len().saturating_sub(page);
    let mut offset = 0usize;
    loop {
        queue!(stdout, Clear(ClearType::All))?;
        draw_box(
            stdout,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            t(" Notice ", " 提示 "),
        )?;
        for (row, line) in lines.iter().skip(offset).take(page).enumerate() {
            queue!(
                stdout,
                MoveTo(
                    frame.x.saturating_add(2),
                    frame.y.saturating_add(2).saturating_add(row as u16)
                ),
                Print(line.clone())
            )?;
        }
        let status = if max_offset == 0 {
            help_line(&[(t("any key", "任意键"), t("continue", "继续"))])
        } else {
            // 滚动时把位置一并显示，否则用户不知道下面还有多少
            format!(
                "{}  {}/{}",
                help_line(&[
                    ("↑↓", t("scroll", "滚动")),
                    (t("any key", "任意键"), t("continue", "继续")),
                ]),
                offset.saturating_add(page).min(lines.len()),
                lines.len()
            )
        };
        draw_status_bar(stdout, &frame, &status)?;
        stdout.flush()?;
        let key = read_key()?;
        let next = match key {
            KeyCode::Up | KeyCode::Char('k') => offset.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => (offset + 1).min(max_offset),
            KeyCode::PageUp => offset.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Char(' ') => (offset + page).min(max_offset),
            KeyCode::Home => 0,
            KeyCode::End => max_offset,
            _ => return Ok(()),
        };
        // 已经到头/到尾时按方向键视为「任意键」直接关闭，避免空按一下没反应
        if next == offset && max_offset > 0 {
            return Ok(());
        }
        offset = next;
    }
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

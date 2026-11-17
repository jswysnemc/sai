//! 配置界面纯文本与带 ANSI 样式文本的列宽测量、折行、底栏压缩及顶栏标题安全裁剪。

use super::super::search::SEARCH_CURSOR_MARK;
use super::super::theme::{help_separator, ThemePalette, BOLD, RESET};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 【配置界面】【ANSI宽度】计算跳过转义序列与零宽标记后的终端可见显示列宽。
///
/// 参数:
/// - `value`: 可能带 ANSI 样式的文本
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
        if ch != SEARCH_CURSOR_MARK {
            width += UnicodeWidthChar::width(ch).unwrap_or(0);
        }
        index += ch.len_utf8();
    }
    width
}

/// 【配置界面】【底栏压缩】按优先级丢弃中间快捷键段，确保窄屏始终保留首段与末尾返回键。
///
/// 参数:
/// - `content`: 已带样式的状态文本
/// - `max`: 可用显示列宽
///
/// 返回:
/// - 压缩后的状态文本
pub(super) fn fit_status_bar(content: &str, max: usize) -> String {
    let measured = ansi_width(content);
    if measured <= max {
        return content.to_string();
    }
    let separator = help_separator();
    let segments = content.split(&separator).collect::<Vec<_>>();
    if segments.len() < 3 {
        return truncate_ansi(content, max);
    }
    let last = segments.len() - 1;
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

/// 【配置界面】【文本折行】按终端显示宽度与显式换行符将文本切分为多行。
///
/// 参数:
/// - `text`: 原始文本
/// - `width`: 单行最大显示宽度
///
/// 返回:
/// - 折行后的字符串向量
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

/// 【配置界面】【标题裁剪】按外框列宽安全截断顶栏标题，确保右上角至少保留 2 列以上边框余量。
///
/// 参数:
/// - `title`: 原始标题（可包含宽字符或 ANSI 转义序列）
/// - `box_width`: 外框总列宽
///
/// 返回:
/// - 可安全嵌入顶边框的已样式化标题片段；极窄框放不下时返回 None
pub(super) fn format_box_title(title: &str, box_width: u16) -> Option<String> {
    // 1. 【配置界面】【余量预算】左侧留 2 列边框、徽标装饰占 4 列、右侧预留至少 3 列边框（兼容宽字符菱形）
    let max_title_cols = (box_width as usize).checked_sub(9)?;
    if max_title_cols == 0 {
        return None;
    }
    let palette = ThemePalette::current();
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return None;
    }
    // 2. 【配置界面】【ANSI与宽字符截断】跳过转义序列统计可见列宽，超长时追加省略号且不破坏右上角边框
    let title_style = format!("{}{BOLD}", palette.accent);
    let fitted = clip_ansi_with_ellipsis(trimmed, max_title_cols, &title_style);
    Some(format!(
        " {}◆{RESET} {title_style}{fitted}{RESET} ",
        palette.brand
    ))
}

/// 【配置界面】【ANSI省略截断】按可见列宽截断可能含 ANSI 序列的文本，超长以 `…` 收尾。
fn clip_ansi_with_ellipsis(value: &str, max: usize, restore_style: &str) -> String {
    if max == 0 {
        return String::new();
    }
    if ansi_width(value) <= max {
        return value.replace(RESET, &format!("{RESET}{restore_style}"));
    }
    if max == 1 {
        return "…".to_string();
    }
    let budget = max - 1;
    let mut width = 0usize;
    let mut out = String::new();
    let mut chars = value.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(value, index);
            let seq = &value[index..end];
            if seq == RESET {
                out.push_str(RESET);
                out.push_str(restore_style);
            } else {
                out.push_str(seq);
            }
            while chars.peek().is_some_and(|(next, _)| *next < end) {
                chars.next();
            }
            continue;
        }
        let char_w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + char_w > budget {
            break;
        }
        out.push(ch);
        width += char_w;
    }
    out.push_str(restore_style);
    out.push('…');
    out
}

/// 【配置界面】【列宽截断】按终端显示列宽截断纯文本，超长追加省略号 `…`。
///
/// 参数:
/// - `value`: 原始纯文本
/// - `max`: 最大显示列宽
///
/// 返回:
/// - 截断后的字符串
pub(crate) fn truncate(value: &str, max: usize) -> String {
    if display_width(value) <= max {
        return value.to_string();
    }
    if max == 0 {
        return String::new();
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

/// 【配置界面】【ANSI截断】按可见宽度截断带 ANSI 样式的文本并以复位收尾。
pub(super) fn truncate_ansi(value: &str, max: usize) -> String {
    let mut width = 0usize;
    let mut output = String::new();
    let mut chars = value.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(value, index);
            output.push_str(&value[index..end]);
            while chars.peek().is_some_and(|(next, _)| *next < end) {
                chars.next();
            }
            continue;
        }
        if ch == SEARCH_CURSOR_MARK {
            output.push(ch);
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

/// 【配置界面】【显示列宽】计算纯文本在终端中占用的显示列宽。
pub(crate) fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

/// 【配置界面】【定宽补齐】将纯文本截断或右侧补空格至指定显示列宽。
pub(crate) fn pad(value: &str, width: usize) -> String {
    let value = truncate(value, width);
    let len = display_width(&value);
    if len >= width {
        value
    } else {
        format!("{value}{}", " ".repeat(width - len))
    }
}

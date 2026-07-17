use crate::question::MAX_CUSTOM_ANSWER_CHARS;
use anyhow::Result;
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 按终端显示宽度对文本做软换行。
///
/// # 参数
/// - `value`: 原始文本
/// - `width`: 每行最大显示宽度
///
/// # 返回值
/// 换行后的文本行
pub(super) fn wrap_display_text(value: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for ch in value.chars() {
        let char_width = ch.width().unwrap_or(0);
        if current_width > 0 && current_width.saturating_add(char_width) > width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width = current_width.saturating_add(char_width);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// 为终端提问面板预留指定行数。
///
/// # 参数
/// - `lines`: 需要预留的行数
///
/// # 返回值
/// 输出成功时返回空结果
pub(super) fn reserve_space(lines: u16) -> Result<()> {
    for _ in 1..lines {
        println!();
    }
    io::stdout().flush()?;
    Ok(())
}

/// 在字符光标位置插入清理后的文本，并限制答案长度。
///
/// # 参数
/// - `value`: 当前文本
/// - `cursor`: 字符光标位置
/// - `text`: 待插入文本
///
/// # 返回值
/// 无返回值
pub(super) fn insert_text(value: &mut String, cursor: &mut usize, text: &str) {
    let remaining = MAX_CUSTOM_ANSWER_CHARS.saturating_sub(value.chars().count());
    if remaining == 0 {
        return;
    }
    let sanitized = text
        .chars()
        .flat_map(|ch| {
            if ch == '\t' {
                "  ".chars().collect::<Vec<_>>()
            } else if ch == '\n' || !ch.is_control() {
                vec![ch]
            } else {
                Vec::new()
            }
        })
        .take(remaining)
        .collect::<String>();
    let byte = byte_index(value, *cursor);
    value.insert_str(byte, &sanitized);
    *cursor += sanitized.chars().count();
}

/// 将多行或控制字符转换为单行可显示文本。
///
/// # 参数
/// - `value`: 原始文本
///
/// # 返回值
/// 清理后的单行文本
pub(super) fn display_inline(value: &str) -> String {
    value
        .chars()
        .filter_map(|ch| match ch {
            '\n' | '\r' => Some('↵'),
            '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect()
}

/// 计算编辑器可见文本和光标显示列。
///
/// # 参数
/// - `value`: 编辑器完整文本
/// - `cursor`: 字符光标位置
/// - `width`: 可用显示宽度
///
/// # 返回值
/// 可见文本和光标列
pub(super) fn editor_view(value: &str, cursor: usize, width: usize) -> (String, usize) {
    if width == 0 {
        return (String::new(), 0);
    }
    let display = display_inline(value);
    let before = display_inline(&value.chars().take(cursor).collect::<String>());
    let cursor_width = UnicodeWidthStr::width(before.as_str());
    if UnicodeWidthStr::width(display.as_str()) <= width {
        return (display, cursor_width.min(width));
    }
    if cursor_width < width {
        return (truncate_plain_width(&display, width), cursor_width);
    }

    let tail_budget = width.saturating_sub(1);
    let mut tail = String::new();
    let mut tail_width = 0usize;
    for ch in before.chars().rev() {
        let ch_width = ch.width().unwrap_or(0);
        if tail_width + ch_width > tail_budget {
            break;
        }
        tail.insert(0, ch);
        tail_width += ch_width;
    }
    let after = display
        .chars()
        .skip(before.chars().count())
        .collect::<String>();
    let mut view = format!("…{tail}");
    let remaining = width.saturating_sub(1 + tail_width);
    view.push_str(&truncate_plain_width(&after, remaining));
    (view, (1 + tail_width).min(width))
}

/// 按显示宽度截断不含终端样式的文本。
///
/// # 参数
/// - `value`: 原始文本
/// - `max_width`: 最大显示宽度
///
/// # 返回值
/// 截断后的文本
fn truncate_plain_width(value: &str, max_width: usize) -> String {
    let mut output = String::new();
    let mut width = 0usize;
    for ch in value.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        output.push(ch);
        width += ch_width;
    }
    output
}

/// 删除字符光标之前的一个字符。
///
/// # 参数
/// - `value`: 当前文本
/// - `cursor`: 字符光标位置
///
/// # 返回值
/// 无返回值
pub(super) fn remove_before_cursor(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let start = byte_index(value, *cursor - 1);
    let end = byte_index(value, *cursor);
    value.replace_range(start..end, "");
    *cursor -= 1;
}

/// 删除字符光标所在位置的一个字符。
///
/// # 参数
/// - `value`: 当前文本
/// - `cursor`: 字符光标位置
///
/// # 返回值
/// 无返回值
pub(super) fn remove_at_cursor(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = byte_index(value, cursor);
    let end = byte_index(value, cursor + 1);
    value.replace_range(start..end, "");
}

/// 将字符索引转换为 UTF-8 字节索引。
///
/// # 参数
/// - `value`: 原始文本
/// - `char_index`: 字符索引
///
/// # 返回值
/// 对应字节索引
fn byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

/// 按显示宽度截断包含 ANSI 样式的文本。
///
/// # 参数
/// - `value`: 原始文本
/// - `max_width`: 最大显示宽度
///
/// # 返回值
/// 截断后的文本
pub(super) fn truncate_width(value: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(strip_ansi(value).as_str()) <= max_width {
        return value.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let budget = max_width.saturating_sub(3);
    let mut output = String::new();
    let mut width = 0usize;
    let mut in_escape = false;
    for ch in value.chars() {
        if ch == '\x1b' {
            in_escape = true;
            output.push(ch);
            continue;
        }
        if in_escape {
            output.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        let char_width = ch.width().unwrap_or(0);
        if width + char_width > budget {
            break;
        }
        output.push(ch);
        width += char_width;
    }
    output.push_str("...\x1b[0m");
    output
}

/// 移除文本中的 ANSI 样式序列。
///
/// # 参数
/// - `value`: 含终端样式的文本
///
/// # 返回值
/// 纯文本内容
pub(super) fn strip_ansi(value: &str) -> String {
    let mut output = String::new();
    let mut in_escape = false;
    for ch in value.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

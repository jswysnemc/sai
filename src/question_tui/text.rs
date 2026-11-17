use crate::question::MAX_CUSTOM_ANSWER_CHARS;
use anyhow::Result;
use std::io::{self, Write};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// ANSI 转义序列扫描状态机。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnsiScanState {
    Normal,
    Esc,
    Csi,
    Osc,
    OscEsc,
    Charset,
}

impl AnsiScanState {
    /// 【终端提问】【转义扫描】推进单个字符并返回该字符是否属于可见正文。
    ///
    /// 参数:
    /// - `self`: 当前扫描状态
    /// - `ch`: 输入字符
    ///
    /// 返回:
    /// - `(下一状态, 是否为可见字符)`
    fn step(self, ch: char) -> (Self, bool) {
        match self {
            Self::Normal => {
                if ch == '\x1b' {
                    (Self::Esc, false)
                } else {
                    (Self::Normal, true)
                }
            }
            Self::Esc => match ch {
                '[' => (Self::Csi, false),
                ']' => (Self::Osc, false),
                '('..='/' => (Self::Charset, false),
                _ => (Self::Normal, false),
            },
            Self::Csi => {
                if ('@'..='~').contains(&ch) {
                    (Self::Normal, false)
                } else {
                    (Self::Csi, false)
                }
            }
            Self::Osc => match ch {
                '\x07' => (Self::Normal, false),
                '\x1b' => (Self::OscEsc, false),
                _ => (Self::Osc, false),
            },
            Self::OscEsc => (Self::Normal, false),
            Self::Charset => (Self::Normal, false),
        }
    }
}

/// 【终端提问】【软换行】按终端显示宽度对纯文本做软换行。
///
/// 参数:
/// - `value`: 原始文本
/// - `width`: 每行最大显示宽度
///
/// 返回:
/// - 换行后的文本行列表
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

/// 【终端提问】【预留空间】为终端提问面板预留指定行数。
///
/// 参数:
/// - `lines`: 需要预留的行数
///
/// 返回:
/// - 输出成功时返回空结果
pub(super) fn reserve_space(lines: u16) -> Result<()> {
    let mut stdout = io::stdout();
    // 1. 【终端提问】【预留空间】清除继承样式并归零列坐标，raw 模式下显式使用 CRLF
    write!(stdout, "\x1b[0m\r")?;
    for _ in 1..lines {
        write!(stdout, "\r\n")?;
    }
    stdout.flush()?;
    Ok(())
}

/// 【终端提问】【文本插入】在字符光标位置插入清理后的文本并限制答案最大长度。
///
/// 参数:
/// - `value`: 当前文本
/// - `cursor`: 字符光标位置
/// - `text`: 待插入文本
///
/// 返回:
/// - 无
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

/// 【终端提问】【单行净化】将多行或控制字符转换为单行可显示文本。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 清理后的单行文本
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

/// 【终端提问】【编辑器视口】计算编辑器可见文本和光标显示列。
///
/// 参数:
/// - `value`: 编辑器完整文本
/// - `cursor`: 字符光标位置
/// - `width`: 可用显示宽度
///
/// 返回:
/// - `(可见文本, 光标显示列)`
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

/// 【终端提问】【纯文本截断】按显示宽度截断不含终端样式的文本。
///
/// 参数:
/// - `value`: 原始文本
/// - `max_width`: 最大显示宽度
///
/// 返回:
/// - 截断后的文本
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

/// 【终端提问】【向前删除】删除字符光标之前的一个字符。
///
/// 参数:
/// - `value`: 当前文本
/// - `cursor`: 字符光标位置
///
/// 返回:
/// - 无
pub(super) fn remove_before_cursor(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let start = byte_index(value, *cursor - 1);
    let end = byte_index(value, *cursor);
    value.replace_range(start..end, "");
    *cursor -= 1;
}

/// 【终端提问】【原位删除】删除字符光标所在位置的一个字符。
///
/// 参数:
/// - `value`: 当前文本
/// - `cursor`: 字符光标位置
///
/// 返回:
/// - 无
pub(super) fn remove_at_cursor(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = byte_index(value, cursor);
    let end = byte_index(value, cursor + 1);
    value.replace_range(start..end, "");
}

/// 【终端提问】【字节定位】将字符索引转换为 UTF-8 字节索引。
///
/// 参数:
/// - `value`: 原始文本
/// - `char_index`: 字符索引
///
/// 返回:
/// - 对应字节索引
fn byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

/// 【终端提问】【样式截断】按显示宽度截断包含 ANSI 转义序列的文本。
///
/// 参数:
/// - `value`: 原始文本
/// - `max_width`: 最大显示宽度
///
/// 返回:
/// - 截断并安全复位后的文本
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
    let mut state = AnsiScanState::Normal;
    // 1. 【终端提问】【状态机截断】仅在完整转义序列之外统计可见列宽，杜绝截断半截控制码
    for ch in value.chars() {
        let (next, is_visible) = state.step(ch);
        if !is_visible {
            state = next;
            output.push(ch);
            continue;
        }
        state = next;
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

/// 【终端提问】【剥离转义】移除文本中的全部 ANSI/CSI/OSC 控制序列。
///
/// 参数:
/// - `value`: 含终端控制序列的文本
///
/// 返回:
/// - 纯文本内容
pub(super) fn strip_ansi(value: &str) -> String {
    let mut output = String::new();
    let mut state = AnsiScanState::Normal;
    for ch in value.chars() {
        let (next, is_visible) = state.step(ch);
        state = next;
        if is_visible {
            output.push(ch);
        }
    }
    output
}

use super::*;

pub(super) fn insert_char_at_cursor(value: &mut String, cursor: &mut usize, ch: char) {
    let byte_index = byte_index_for_char(value, *cursor);
    value.insert(byte_index, ch);
    *cursor += 1;
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn insert_str_at_cursor(value: &mut String, cursor: &mut usize, text: &str) {
    let byte_index = byte_index_for_char(value, *cursor);
    value.insert_str(byte_index, text);
    *cursor += text.chars().count();
}

pub(super) fn insert_newline_at_cursor(value: &mut String, cursor: &mut usize) {
    insert_char_at_cursor(value, cursor, '\n');
}

pub(super) fn remove_char_before_cursor(value: &mut String, cursor: &mut usize) {
    let end = byte_index_for_char(value, *cursor);
    let start = byte_index_for_char(value, cursor.saturating_sub(1));
    value.replace_range(start..end, "");
    *cursor -= 1;
}

pub(super) fn remove_word_before_cursor(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let chars = value.chars().collect::<Vec<_>>();
    let mut start = (*cursor).min(chars.len());
    while start > 0 && chars[start - 1].is_whitespace() {
        start -= 1;
    }
    while start > 0 && !chars[start - 1].is_whitespace() {
        start -= 1;
    }
    let byte_start = byte_index_for_char(value, start);
    let byte_end = byte_index_for_char(value, *cursor);
    value.replace_range(byte_start..byte_end, "");
    *cursor = start;
}

pub(super) fn remove_char_at_cursor(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = byte_index_for_char(value, cursor);
    let end = byte_index_for_char(value, cursor + 1);
    value.replace_range(start..end, "");
}

pub(super) fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

pub(super) fn take_chars(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}

pub(super) fn terminal_cols() -> usize {
    terminal::size()
        .map(|(cols, _)| cols.max(1) as usize)
        .unwrap_or(80)
}

pub(super) fn repl_input_lines(input: &str) -> Vec<String> {
    let normalized = strip_terminal_control_sequences(input)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines = normalized
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

pub(super) fn strip_terminal_control_sequences(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek() {
                // 1. CSI：吞到最终字节 @-~
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                // 2. OSC 与 DCS：吞到 BEL 或 ST（ESC \），否则标题等负载会残留进输入
                Some(']') | Some('P') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                // 3. SS3 等双字节序列：连同后一个字符一起吞掉
                Some('O') | Some('N') => {
                    chars.next();
                    chars.next();
                }
                _ => {
                    chars.next();
                }
            }
            continue;
        }
        if is_disallowed_control_char(ch) {
            continue;
        }
        output.push(ch);
    }
    output
}

pub(super) fn is_disallowed_control_char(ch: char) -> bool {
    ch.is_control() && !matches!(ch, '\n' | '\t')
}

/// 计算文本在终端中的可见宽度。
///
/// 宽度口径统一走 unicode-width，与 transcript 渲染一致；
/// ANSI 转义序列按 CSI 语法整段跳过，不再假设只有 SGR。
///
/// 参数:
/// - `value`: 待测量文本，可含 ANSI 转义
///
/// 返回:
/// - 可见列数
pub(super) fn visible_width(value: &str) -> usize {
    visible_chars(value).map(char_terminal_width).sum()
}

/// 计算单个字符的终端显示宽度。
///
/// 参数:
/// - `ch`: 待测量字符
///
/// 返回:
/// - 显示列数；组合字符与零宽字符为 0
pub(super) fn char_terminal_width(ch: char) -> usize {
    use unicode_width::UnicodeWidthChar;
    ch.width().unwrap_or(0)
}

/// 迭代文本中的可见字符，跳过 ANSI 转义序列。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 可见字符迭代器
fn visible_chars(value: &str) -> impl Iterator<Item = char> + '_ {
    let mut in_csi = false;
    let mut pending_escape = false;
    value.chars().filter_map(move |ch| {
        if in_csi {
            if ('@'..='~').contains(&ch) {
                in_csi = false;
            }
            return None;
        }
        if pending_escape {
            pending_escape = false;
            if ch == '[' {
                in_csi = true;
            }
            return None;
        }
        if ch == '\x1b' {
            pending_escape = true;
            return None;
        }
        Some(ch)
    })
}

/// 终端自动换行模拟器的光标状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WrapPosition {
    pub col: usize,
    pub row: usize,
}

/// 模拟一段文本写入终端后的光标落点。
///
/// 与纯 `宽度/列数` 模运算的关键区别：宽字符放不下行尾最后一列时，
/// 终端会把整个字符移到下一行并在行尾留一个空列；制表符前进到
/// 下一个 8 列制表位且不越过右边界。中英混排的行数与光标列
/// 必须按这套规则逐字符推进，否则每次跨界都会偏移一列。
///
/// 参数:
/// - `prefix`: 行首前缀（提示符），参与占位
/// - `text`: 单个逻辑行文本（不含换行符），可含 ANSI 转义
/// - `cols`: 终端列数
///
/// 返回:
/// - 末字符之后的光标位置；恰好填满整行时归一化为下一行行首
pub(super) fn wrapped_end_position(prefix: &str, text: &str, cols: usize) -> WrapPosition {
    let cols = cols.max(1);
    let mut position = WrapPosition { col: 0, row: 0 };
    for ch in visible_chars(prefix).chain(visible_chars(text)) {
        // 1. 制表符：前进到下一个 8 列制表位，止步于右边界
        if ch == '\t' {
            position.col = ((position.col / 8 + 1) * 8).min(cols.saturating_sub(1));
            continue;
        }
        let width = char_terminal_width(ch);
        if width == 0 {
            continue;
        }
        // 2. 放不下时整字符移到下一行，行尾留空列
        if position.col + width > cols {
            position.row += 1;
            position.col = 0;
        }
        position.col += width;
    }
    // 3. 恰好填满整行时终端处于悬挂换行态，下一个落点是下一行行首
    if position.col >= cols {
        position.row += 1;
        position.col = 0;
    }
    position
}

/// 计算一个逻辑行占用的视觉行数。
///
/// 行数与光标共用同一模拟器：行尾光标落点所在的行也计入，
/// 因此恰好填满整行的文本会多出一行，为光标预留位置。
///
/// 参数:
/// - `prefix`: 行首前缀
/// - `text`: 单个逻辑行文本
/// - `cols`: 终端列数
///
/// 返回:
/// - 视觉行数，至少为 1
pub(super) fn wrapped_line_rows(prefix: &str, text: &str, cols: usize) -> usize {
    wrapped_end_position(prefix, text, cols).row + 1
}

#[allow(dead_code)]
pub(super) fn colored_mode_label(mode: AgentMode) -> String {
    match mode {
        AgentMode::Yolo => "\x1b[38;5;208m[YOLO]\x1b[0m".to_string(),
        AgentMode::Audited => "\x1b[35m[AUDIT]\x1b[0m".to_string(),
        AgentMode::AutoAudit => "\x1b[38;5;141m[AUTO-AUDIT]\x1b[0m".to_string(),
        AgentMode::Plan => "\x1b[36m[PLAN]\x1b[0m".to_string(),
    }
}

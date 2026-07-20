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
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            } else {
                chars.next();
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

pub(super) fn visible_width(value: &str) -> usize {
    let mut width = 0usize;
    let mut escape = false;
    for ch in value.chars() {
        if escape {
            if ch == 'm' {
                escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            escape = true;
        } else if (ch as u32) >= 0x2e80 {
            width += 2;
        } else {
            width += 1;
        }
    }
    width
}

#[allow(dead_code)]
pub(super) fn colored_mode_label(mode: AgentMode) -> String {
    match mode {
        AgentMode::Yolo => "\x1b[38;5;208m[YOLO]\x1b[0m".to_string(),
        AgentMode::Audited => "\x1b[35m[AUDIT]\x1b[0m".to_string(),
        AgentMode::AutoAudit => "\x1b[38;5;141m[AUTO]\x1b[0m".to_string(),
        AgentMode::Plan => "\x1b[36m[PLAN]\x1b[0m".to_string(),
    }
}

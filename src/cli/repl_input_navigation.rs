/// 将光标移动到上一条视觉行。
///
/// 参数:
/// - `prefix`: 首行提示符文本
/// - `input`: 当前输入内容
/// - `cursor`: 当前光标字符索引
/// - `cols`: 终端列数
///
/// 返回:
/// - 上一条视觉行中的目标光标字符索引，首行时返回空
pub(super) fn move_cursor_up_by_visual_row(
    prefix: &str,
    input: &str,
    cursor: usize,
    cols: usize,
) -> Option<usize> {
    move_cursor_by_visual_row(prefix, input, cursor, cols, -1)
}

/// 将光标移动到下一条视觉行。
///
/// 参数:
/// - `prefix`: 首行提示符文本
/// - `input`: 当前输入内容
/// - `cursor`: 当前光标字符索引
/// - `cols`: 终端列数
///
/// 返回:
/// - 下一条视觉行中的目标光标字符索引，末行时返回空
pub(super) fn move_cursor_down_by_visual_row(
    prefix: &str,
    input: &str,
    cursor: usize,
    cols: usize,
) -> Option<usize> {
    move_cursor_by_visual_row(prefix, input, cursor, cols, 1)
}

/// 按视觉行移动光标。
///
/// 参数:
/// - `prefix`: 首行提示符文本
/// - `input`: 当前输入内容
/// - `cursor`: 当前光标字符索引
/// - `cols`: 终端列数
/// - `direction`: 移动方向，-1 表示向上，1 表示向下
///
/// 返回:
/// - 目标光标字符索引，越界时返回空
fn move_cursor_by_visual_row(
    prefix: &str,
    input: &str,
    cursor: usize,
    cols: usize,
    direction: i16,
) -> Option<usize> {
    let positions = cursor_positions(prefix, input, cols);
    let current = positions
        .iter()
        .find(|position| position.cursor == cursor.min(input.chars().count()))?;
    let max_row = positions
        .iter()
        .map(|position| position.row)
        .max()
        .unwrap_or(0);
    let target_row = match direction {
        -1 if current.row > 0 => current.row - 1,
        1 if current.row < max_row => current.row + 1,
        _ => return None,
    };
    positions
        .iter()
        .filter(|position| position.row == target_row)
        .min_by_key(|position| {
            let distance = position.col.abs_diff(current.col);
            let after_target = usize::from(position.col > current.col);
            (distance, after_target, position.cursor)
        })
        .map(|position| position.cursor)
}

#[derive(Debug, Clone, Copy)]
struct CursorPosition {
    cursor: usize,
    col: usize,
    row: usize,
}

/// 生成每个字符索引对应的视觉光标位置。
///
/// 参数:
/// - `prefix`: 首行提示符文本
/// - `input`: 当前输入内容
/// - `cols`: 终端列数
///
/// 返回:
/// - 光标位置列表
fn cursor_positions(prefix: &str, input: &str, cols: usize) -> Vec<CursorPosition> {
    let char_count = input.chars().count();
    (0..=char_count)
        .map(|cursor| {
            let (col, row) = cursor_position_for_cols(prefix, input, cursor, cols);
            CursorPosition { cursor, col, row }
        })
        .collect()
}

/// 计算指定字符索引在终端中的视觉位置。
///
/// 参数:
/// - `prefix`: 首行提示符文本
/// - `input`: 当前输入内容
/// - `cursor`: 当前光标字符索引
/// - `cols`: 终端列数
///
/// 返回:
/// - 视觉列和视觉行
fn cursor_position_for_cols(
    prefix: &str,
    input: &str,
    cursor: usize,
    cols: usize,
) -> (usize, usize) {
    let cols = cols.max(1);
    let before_cursor = input.chars().take(cursor).collect::<String>();
    let lines = input_lines(&before_cursor);
    let last_index = lines.len().saturating_sub(1);
    let mut row_offset = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let width = if index == 0 {
            visible_width(prefix) + visible_width(line)
        } else {
            visible_width(line)
        };
        if index == last_index {
            return (width % cols, row_offset + width / cols);
        }
        row_offset += width / cols + 1;
    }
    (visible_width(prefix), 0)
}

/// 按换行符拆分输入。
///
/// 参数:
/// - `input`: 当前输入内容
///
/// 返回:
/// - 输入行列表
fn input_lines(input: &str) -> Vec<String> {
    let mut lines = input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// 计算终端可见宽度。
///
/// 参数:
/// - `value`: 输入文本
///
/// 返回:
/// - 可见宽度
fn visible_width(value: &str) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_between_explicit_lines() {
        assert_eq!(move_cursor_up_by_visual_row("", "one\ntwo", 7, 80), Some(3));
        assert_eq!(
            move_cursor_down_by_visual_row("", "one\ntwo", 3, 80),
            Some(7)
        );
    }

    #[test]
    fn returns_none_at_input_edges() {
        assert_eq!(move_cursor_up_by_visual_row("", "one\ntwo", 0, 80), None);
        assert_eq!(move_cursor_down_by_visual_row("", "one\ntwo", 7, 80), None);
    }

    #[test]
    fn moves_between_wrapped_visual_lines() {
        assert_eq!(move_cursor_up_by_visual_row("", "abcdef", 6, 5), Some(1));
        assert_eq!(move_cursor_down_by_visual_row("", "abcdef", 1, 5), Some(6));
    }

    #[test]
    fn accounts_for_prompt_prefix_on_first_line() {
        assert_eq!(
            move_cursor_down_by_visual_row("[YOLO] > ", "abc\ndef", 3, 80),
            Some(7)
        );
        assert_eq!(
            move_cursor_up_by_visual_row("[YOLO] > ", "abc\ndef", 7, 80),
            Some(0)
        );
    }
}

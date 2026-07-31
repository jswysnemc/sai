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
/// 单次遍历增量推进，与渲染层共用同一换行模拟规则
/// （宽字符跨界整体下移、制表符止步右边界、整行填满后悬挂到下一行行首），
/// 避免每个索引重扫全文的平方开销。
///
/// 参数:
/// - `prefix`: 首行提示符文本
/// - `input`: 当前输入内容
/// - `cols`: 终端列数
///
/// 返回:
/// - 光标位置列表
fn cursor_positions(prefix: &str, input: &str, cols: usize) -> Vec<CursorPosition> {
    let cols = cols.max(1);
    let start = super::repl_text::wrapped_end_position(prefix, "", cols);
    let mut positions = Vec::with_capacity(input.chars().count() + 1);
    let mut col = start.col;
    let mut row = start.row;
    positions.push(CursorPosition {
        cursor: 0,
        col,
        row,
    });
    for (index, ch) in input.chars().enumerate() {
        match ch {
            // 1. 逻辑换行：新行从下一视觉行行首开始
            '\n' | '\r' => {
                row += 1;
                col = 0;
            }
            // 2. 制表符：前进到下一个 8 列制表位，止步于右边界
            '\t' => {
                col = ((col / 8 + 1) * 8).min(cols.saturating_sub(1));
            }
            _ => {
                let width = super::repl_text::char_terminal_width(ch);
                if width > 0 {
                    // 3. 宽字符放不下行尾时整体移到下一行
                    if col + width > cols {
                        row += 1;
                        col = 0;
                    }
                    col += width;
                }
            }
        }
        // 4. 恰好填满整行时光标悬挂到下一行行首，与渲染层归一化一致
        if col >= cols {
            row += 1;
            col = 0;
        }
        positions.push(CursorPosition {
            cursor: index + 1,
            col,
            row,
        });
    }
    positions
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

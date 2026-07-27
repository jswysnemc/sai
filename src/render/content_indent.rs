/// 终端视觉引导线占用的列宽。
pub(crate) const GUIDE_COLUMN_WIDTH: usize = 1;
/// 正文与视觉引导线之间保留的空白列宽。
pub(crate) const GUIDE_CONTENT_GAP_WIDTH: usize = 1;
/// TUI 正文相对终端左边界的总缩进。
pub(crate) const CONTENT_LEFT_INDENT: usize = GUIDE_COLUMN_WIDTH + GUIDE_CONTENT_GAP_WIDTH;
/// diff 相对正文再向内收的列宽。
pub(crate) const DIFF_NESTED_INDENT: usize = 1;
/// diff 相对终端左右边界的总内收宽度。
pub(crate) const DIFF_BLOCK_INSET: usize = CONTENT_LEFT_INDENT + DIFF_NESTED_INDENT;

/// 将一行内容放到视觉引导线两侧的正确列。
///
/// 参数:
/// - `text`: 已完成 ANSI 渲染的终端行
///
/// 返回:
/// - 引导符号位于左侧，普通正文位于右侧的终端行
pub(crate) fn align_to_guide_column(text: &str) -> String {
    let (leading_spaces, first_visible) = visible_line_start(text);
    if matches!(first_visible, Some('●' | '•')) && leading_spaces == 0 {
        return text.to_string();
    }
    if text.contains("\x1b[K") {
        let raw_indent = text.bytes().take_while(|byte| *byte == b' ').count();
        let indent = if raw_indent == 0 {
            DIFF_BLOCK_INSET
        } else {
            CONTENT_LEFT_INDENT
        };
        return format!("{}{text}", " ".repeat(indent));
    }
    let indent = match leading_spaces {
        // 无缩进正文移动到引导线右侧
        0 => CONTENT_LEFT_INDENT,
        // 一列缩进表示 diff 内层，叠加正文基线后共三列
        1 => CONTENT_LEFT_INDENT,
        // cell 已经为续行预留正文基线，不重复添加
        _ => 0,
    };
    format!("{}{text}", " ".repeat(indent))
}

/// 为 TUI diff 的每一行增加正文内部缩进。
///
/// 参数:
/// - `text`: 原始 diff 文本块
///
/// 返回:
/// - 每一行向正文内部再移动一列的文本
pub(crate) fn indent_diff_for_transcript(text: &str) -> String {
    indent_lines(text, DIFF_NESTED_INDENT)
}

/// 为 CLI diff 的每一行增加完整块级缩进。
///
/// 参数:
/// - `text`: 原始 diff 文本块
///
/// 返回:
/// - 每一行按 diff 总内收宽度移动后的文本
pub(crate) fn indent_diff_for_cli(text: &str) -> String {
    indent_lines(text, DIFF_BLOCK_INSET)
}

/// 生成清除终端最右侧 diff 边距的 ANSI 控制序列。
///
/// 该序列保存当前位置，移动到终端右边界，清除指定列数后恢复光标。
/// 不写入固定宽度空格，因此终端缩放时不会留下碎片色块。
///
/// 参数:
/// - `columns`: 需要恢复为终端默认背景的右侧列数
///
/// 返回:
/// - 清除右侧边距并恢复光标位置的 ANSI 序列
pub(crate) fn clear_right_margin(columns: usize) -> String {
    if columns == 0 {
        return String::new();
    }
    let move_left = columns.saturating_sub(1);
    format!("\x1b[s\x1b[999C\x1b[{move_left}D\x1b[{columns}X\x1b[u")
}

/// 为文本块的每一行增加指定列数的缩进。
///
/// 参数:
/// - `text`: 原始文本块
/// - `columns`: 缩进列数
///
/// 返回:
/// - 每一行增加缩进后的文本
fn indent_lines(text: &str, columns: usize) -> String {
    let indent = " ".repeat(columns);
    text.split_inclusive('\n')
        .map(|line| format!("{indent}{line}"))
        .collect()
}

/// 读取忽略 ANSI 控制序列后的行首空格与首个可见字符。
///
/// 参数:
/// - `text`: ANSI 终端行
///
/// 返回:
/// - 行首可见空格数与首个非空格字符
fn visible_line_start(text: &str) -> (usize, Option<char>) {
    let mut index = 0usize;
    let mut leading_spaces = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if ch == '\x1b' {
            index = crate::render::terminal_image::escape_sequence_end(text, index)
                .max(index + ch.len_utf8());
            continue;
        }
        if ch == ' ' {
            leading_spaces += 1;
            index += ch.len_utf8();
            continue;
        }
        return (leading_spaces, Some(ch));
    }
    (leading_spaces, None)
}

#[cfg(test)]
mod tests {
    use super::{
        align_to_guide_column, clear_right_margin, indent_diff_for_cli, indent_diff_for_transcript,
    };

    /// 引导符号保留在左侧，普通正文与续行位于右侧。
    #[test]
    fn aligns_symbols_and_content_on_opposite_sides() {
        assert_eq!(align_to_guide_column("a"), "  a");
        assert_eq!(align_to_guide_column("  continuation"), "  continuation");
        assert_eq!(
            align_to_guide_column("\x1b[36m●\x1b[0m input"),
            "\x1b[36m●\x1b[0m input"
        );
        assert_eq!(align_to_guide_column("• tool"), "• tool");
        assert_eq!(align_to_guide_column(" diff"), "   diff");
        assert_eq!(
            align_to_guide_column("\x1b[48;5;22m+line\x1b[K\x1b[0m"),
            "   \x1b[48;5;22m+line\x1b[K\x1b[0m"
        );
        assert_eq!(indent_diff_for_transcript("a\nb"), " a\n b");
        assert_eq!(indent_diff_for_cli("a\nb"), "   a\n   b");
    }

    /// 右侧边距序列必须清除目标列数并恢复原光标。
    #[test]
    fn right_margin_clear_preserves_cursor_position() {
        let sequence = clear_right_margin(3);
        assert!(sequence.starts_with("\x1b[s"));
        assert!(sequence.contains("\x1b[2D\x1b[3X"));
        assert!(sequence.ends_with("\x1b[u"));
    }
}

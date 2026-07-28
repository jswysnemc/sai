use crossterm::terminal;

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

/// 【终端】【CLI 布局】将流式输出块放到与 TUI 相同的视觉引导列。
///
/// 光标移动与清行序列继续从终端左边界执行；实际正文随后移动到引导列右侧，
/// `●` 和 `•` 等引导符号保留在左侧。
///
/// 参数:
/// - `text`: 可能包含 ANSI 光标控制序列的流式输出块
///
/// 返回:
/// - 已按视觉引导列对齐的终端文本
pub(crate) fn align_cli_stream_block(text: &str) -> String {
    text.split_inclusive('\n')
        .map(align_cli_stream_line)
        .collect()
}

/// 【终端】【CLI 布局】对齐可能跨多个分片到达的纯文本正文。
///
/// 仅在物理行起点增加正文缩进，避免模型分片位于同一行中间时重复插入空格。
///
/// 参数:
/// - `text`: 当前纯文本增量
/// - `at_line_start`: 调用前是否位于物理行起点；调用后更新为最新状态
///
/// 返回:
/// - 已按视觉引导列对齐的纯文本增量
pub(crate) fn align_cli_text_delta(text: &str, at_line_start: &mut bool) -> String {
    let mut output = String::new();
    for segment in text.split_inclusive('\n') {
        let (body, newline) = segment
            .strip_suffix('\n')
            .map_or((segment, ""), |body| (body, "\n"));
        if *at_line_start && !body.is_empty() {
            output.push_str(&" ".repeat(CONTENT_LEFT_INDENT));
        }
        output.push_str(body);
        output.push_str(newline);
        *at_line_start = !newline.is_empty();
    }
    output
}

/// 【终端】【CLI 布局】返回 CLI 正文区域可使用的渲染宽度。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 终端总宽度扣除引导列与间隔后的列数
pub(crate) fn cli_content_width() -> usize {
    terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100)
        .saturating_sub(CONTENT_LEFT_INDENT)
        .max(1)
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

/// 【终端】【CLI 布局】对齐单个物理行，并保留开头的光标控制序列。
///
/// 参数:
/// - `line`: 包含可选结尾换行的终端文本行
///
/// 返回:
/// - 光标控制仍位于行首、正文已经对齐的终端行
fn align_cli_stream_line(line: &str) -> String {
    let (body, newline) = line
        .strip_suffix('\n')
        .map_or((line, ""), |body| (body, "\n"));
    let control_end = leading_cursor_control_end(body);
    let (controls, content) = body.split_at(control_end);
    if content.is_empty() {
        return line.to_string();
    }
    format!("{controls}{}{newline}", align_to_guide_column(content))
}

/// 【终端】【CLI 布局】定位行首光标移动与清理序列的结束位置。
///
/// SGR 颜色序列属于正文样式，不在此剥离；终端图片协议同样作为正文处理。
///
/// 参数:
/// - `text`: 单个物理行
///
/// 返回:
/// - 行首控制序列后的字节偏移
fn leading_cursor_control_end(text: &str) -> usize {
    let mut index = 0usize;
    while index < text.len() {
        if text.as_bytes().get(index) == Some(&b'\r') {
            index += 1;
            continue;
        }
        if !text[index..].starts_with("\x1b[") {
            break;
        }
        let end = crate::render::terminal_image::escape_sequence_end(text, index);
        if end <= index || text.as_bytes().get(end.saturating_sub(1)) == Some(&b'm') {
            break;
        }
        index = end;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::{
        align_cli_stream_block, align_cli_text_delta, align_to_guide_column, clear_right_margin,
        indent_diff_for_cli, indent_diff_for_transcript,
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

    /// 【终端】【CLI 布局】验证普通正文右移，引导符号保留在左侧。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn aligns_cli_body_without_moving_leading_markers() {
        let block = "answer\n\x1b[36m•\x1b[0m tool\n";

        assert_eq!(
            align_cli_stream_block(block),
            "  answer\n\x1b[36m•\x1b[0m tool\n"
        );
    }

    /// 【终端】【CLI 布局】验证表格重绘控制先执行，再缩进新正文。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn preserves_cursor_controls_before_aligned_content() {
        let block = "\x1b[1A\r\x1b[2Ktable\n";

        assert_eq!(
            align_cli_stream_block(block),
            "\x1b[1A\r\x1b[2K  table\n"
        );
    }

    /// 【终端】【CLI 布局测试】验证纯文本分片只在物理行起点缩进。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn aligns_fragmented_plain_text_without_midline_spaces() {
        let mut at_line_start = true;

        assert_eq!(
            align_cli_text_delta("partial", &mut at_line_start),
            "  partial"
        );
        assert!(!at_line_start);
        assert_eq!(
            align_cli_text_delta(" answer\nnext\n", &mut at_line_start),
            " answer\n  next\n"
        );
        assert!(at_line_start);
    }
}

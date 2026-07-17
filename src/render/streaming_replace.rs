use crate::render::table;
use crossterm::terminal;

/// 计算原始文本行在终端中的视觉行数。
///
/// 参数:
/// - `line`: 已经写入终端的原始文本行
///
/// 返回:
/// - 终端自动换行后占用的视觉行数量
pub(crate) fn raw_visual_rows(line: &str) -> usize {
    let terminal_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100)
        .max(1);
    table::visible_width(line).max(1).div_ceil(terminal_width)
}

/// 生成清除已渲染视觉行的终端控制序列。
///
/// 参数:
/// - `row_count`: 已渲染的视觉行数
///
/// 返回:
/// - 上移并清除每一行的 ANSI 控制序列
pub(crate) fn clear_rendered_rows(row_count: usize) -> String {
    let mut output = String::new();
    for _ in 0..row_count {
        output.push_str("\x1b[1A\r\x1b[2K");
    }
    output
}

/// 计算一段已渲染终端文本占用的视觉行数。
///
/// 参数:
/// - `text`: 已写入或准备写入终端的文本
///
/// 返回:
/// - 终端自动换行后的视觉行数
pub(crate) fn rendered_visual_rows(text: &str) -> usize {
    text.lines().map(raw_visual_rows).sum::<usize>()
}

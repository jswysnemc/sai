use crate::render::table;
use crossterm::terminal;

/// 计算原始文本行在终端中的视觉行数。
///
/// 折行宽度必须由调用方给出：正文最终会经过 `wrap_cli_stream_block`
/// 在 `cli_content_width()`（终端列数减引导区）处折行，之后再补引导区缩进。
/// 这里若按终端整宽去除带缩进的整行，算出的行数会偏小——宽表流式推进时
/// 每帧少清一行，旧行残影会一直往上堆。
///
/// 参数:
/// - `line`: 待写入终端的**未对齐**文本行
/// - `width`: 该行实际会被折行的列宽
///
/// 返回:
/// - 折行后占用的视觉行数量
pub(crate) fn raw_visual_rows(line: &str, width: usize) -> usize {
    let width = width.max(1);
    table::visible_width(line).max(1).div_ceil(width)
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
/// - `width`: 文本实际会被折行的列宽
///
/// 返回:
/// - 折行后的视觉行数
pub(crate) fn rendered_visual_rows(text: &str, width: usize) -> usize {
    text.lines()
        .map(|line| raw_visual_rows(line, width))
        .sum::<usize>()
}

/// 返回终端整列宽度，供直接写满整行的调用方计算视觉行数。
///
/// 返回:
/// - 终端列数（查询失败时为 100）
pub(crate) fn terminal_width() -> usize {
    terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::{raw_visual_rows, rendered_visual_rows};

    /// 行数按调用方给出的折行宽度算，与终端整宽无关。
    #[test]
    fn raw_visual_rows_divides_by_the_given_wrap_width() {
        // 10 列宽、宽 25 的文本 → 3 行；按整宽算会错
        assert_eq!(raw_visual_rows(&"x".repeat(25), 10), 3);
        assert_eq!(raw_visual_rows("abc", 10), 1);
        assert_eq!(raw_visual_rows("", 10), 1);
    }

    /// 回归：宽表原文行按内容宽（cols-2）折行，按整宽（cols）算会少算一行。
    ///
    /// 80 列终端、157 列宽的原文行：内容宽 78 → 3 行；整宽 80 → 2 行。
    /// 少清的那一行会作为残影一直留在屏幕上。
    #[test]
    fn narrow_content_width_counts_more_rows_than_full_terminal_width() {
        let line = "x".repeat(157);
        assert_eq!(raw_visual_rows(&line, 78), 3);
        assert_eq!(raw_visual_rows(&line, 80), 2);
    }

    /// 多行文本按行累加。
    #[test]
    fn rendered_visual_rows_sums_per_line() {
        let text = format!("{}\n{}", "x".repeat(25), "y".repeat(5));
        assert_eq!(rendered_visual_rows(&text, 10), 4);
    }
}

//! 行内差异：稳定前缀后的追加允许局部覆盖，填充空格与右侧轨道不重写。

use super::styled_row;
use crossterm::{
    cursor::MoveTo,
    queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io::{self, Write};

/// 【终端】【行内绘制】样式和宽字符可能变化时返回 false，由调用方整行覆盖。
/// 参数：输出、行号、列数、新旧行；返回：是否完成局部绘制。
pub(super) fn paint_append<W: Write>(
    output: &mut W,
    row: u16,
    cols: usize,
    old: &str,
    new: &str,
) -> io::Result<bool> {
    let Some(old_segments) = styled_row::parse(old) else {
        return Ok(false);
    };
    let Some(new_segments) = styled_row::parse(new) else {
        return Ok(false);
    };
    let Some(span) = styled_row::append_span(&old_segments, &new_segments) else {
        return Ok(false);
    };
    let width = new_segments
        .iter()
        .map(|segment| segment.width)
        .sum::<usize>();
    if span.column >= cols || width > cols || cols > usize::from(u16::MAX) {
        return Ok(false);
    }
    let first = &new_segments[span.first];
    let last = &new_segments[span.end - 1];
    // 1. 【终端】【行内绘制】重建新增字符处的样式，只输出新增字节，不重复旧正文
    queue!(
        output,
        MoveTo(span.column as u16, row),
        Print("\x1b[0m"),
        Print(&first.style),
        Print(&new[first.start..last.start + last.text.len()]),
        Print("\x1b[0m")
    )?;
    if width < cols && span.end == new_segments.len() {
        queue!(output, Clear(ClearType::UntilNewLine))?;
    } else if new_segments[..span.end]
        .iter()
        .map(|segment| segment.width)
        .sum::<usize>()
        == cols
    {
        // 2. 【终端】【行内绘制】满宽追加同样需要取消延迟换行，避免恢复自动换行时滚屏
        queue!(output, MoveTo((cols - 1) as u16, row))?;
    }
    Ok(true)
}

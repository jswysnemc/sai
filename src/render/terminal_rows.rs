use crate::render::activity_animation::strip_ansi_for_test;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

mod diff;
mod styled_row;

/// 【终端】【逐行绘制】只输出发生变化的视觉行，并在写完内容后清除旧行尾。
///
/// 参数: `output` 为帧缓冲，`top` 为首行位置，`cols` 为列数，`lines` 为新行，`previous` 为同位置旧行
/// 返回: 输出结果；相同行不产生任何绘制指令
pub(crate) fn paint_changed_rows<W: Write>(
    output: &mut W,
    top: u16,
    cols: usize,
    lines: &[String],
    previous: Option<&[String]>,
) -> io::Result<()> {
    let previous = previous.unwrap_or_default();
    let mut started = false;
    for index in 0..lines.len().max(previous.len()) {
        if lines.get(index) == previous.get(index) {
            continue;
        }
        if !started {
            // 1. 满宽行采用绝对定位，禁止终端在右下角自行滚屏
            queue!(output, Print("\x1b[?7l"))?;
            started = true;
        }
        let line = lines.get(index).map(String::as_str).unwrap_or_default();
        if let Some(previous) = previous.get(index) {
            if diff::paint_append(
                output,
                top.saturating_add(index as u16),
                cols,
                previous,
                line,
            )? {
                continue;
            }
        }
        queue!(
            output,
            MoveTo(0, top.saturating_add(index as u16)),
            Print(line),
            Print("\x1b[0m")
        )?;
        // 2. 先覆盖正文再清除残留，避免不支持同步更新的终端出现空白中间态
        let width = UnicodeWidthStr::width(strip_ansi_for_test(line).as_str());
        if width < cols {
            queue!(output, Clear(ClearType::UntilNewLine))?;
        } else if cols > 0 {
            // 3. 满宽行在 Windows 上会留下延迟换行。先钉回最后一个单元格，
            //    后面恢复自动换行时才不会把这一行再滚上去。
            queue!(
                output,
                MoveTo((cols - 1) as u16, top.saturating_add(index as u16))
            )?;
        }
    }
    if started {
        queue!(output, Print("\x1b[?7h"))?;
    }
    Ok(())
}

/// 满宽行写完后把光标钉回最后一个单元格，取消 Windows 的延迟换行。
///
/// 参数: `output` 为帧缓冲，`row` 为屏幕行，`cols` 为可见列数，`line` 为刚写过的行
/// 返回: 输出结果；未满宽时不移动光标
pub(crate) fn park_if_full_line<W: Write>(
    output: &mut W,
    row: u16,
    cols: u16,
    line: &str,
) -> io::Result<()> {
    let cols = usize::from(cols);
    if cols == 0 {
        return Ok(());
    }
    let width = UnicodeWidthStr::width(strip_ansi_for_test(line).as_str());
    if width >= cols {
        queue!(output, MoveTo((cols - 1) as u16, row))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;

use crate::render::activity_animation::strip_ansi_for_test;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};
use unicode_width::UnicodeWidthStr;

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
        }
    }
    if started {
        queue!(output, Print("\x1b[?7h"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【逐行绘制】缩短正文后清理行尾，但不能清空整屏或重写相同行。
    /// 参数: 无
    /// 返回: 无，顺序或绘制范围错误时断言失败
    #[test]
    fn paints_only_changes_before_erasing_stale_suffixes() {
        let previous = vec!["stable".into(), "old long value".into()];
        let current = vec!["stable".into(), "new".into()];
        let mut output = Vec::new();
        paint_changed_rows(&mut output, 3, 40, &current, Some(&previous)).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains("stable"));
        assert!(!output.contains("\x1b[2J") && !output.contains("\x1b[2K"));
        assert!(output.find("new").unwrap() < output.find("\x1b[K").unwrap());
    }

    /// 【终端】【逐行绘制】满宽文本不能在最后一列执行清行，否则最后一个字符会消失。
    /// 参数: 无
    /// 返回: 无，覆盖最后一列时断言失败
    #[test]
    fn full_width_row_does_not_erase_its_last_character() {
        let mut output = Vec::new();
        paint_changed_rows(&mut output, 0, 4, &["abcd".into()], None).unwrap();
        assert!(!String::from_utf8(output).unwrap().contains("\x1b[K"));
    }
}

use super::history_insert::replay_lines;
use super::viewport::InlineViewport;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

/// 清空终端 scrollback 的控制序列（ED 3）。
const CLEAR_SCROLLBACK: &str = "\x1b[3J";
/// 输出期间关闭终端自动换行。
const DISABLE_AUTOWRAP: &str = "\x1b[?7l";
/// 恢复终端自动换行。
const ENABLE_AUTOWRAP: &str = "\x1b[?7h";

/// 从 source 行重放历史可视尾部，并清理其后的旧内容。
///
/// 先逐行重绘（每行独立清行），再从最后一行之后清到屏幕底部，
/// 避免整屏先清空再绘制造成的闪烁。
///
/// 参数:
/// - `stdout`: 终端输出句柄
/// - `viewport`: 当前 inline viewport
/// - `lines`: 当前宽度下的预换行 transcript 行
///
/// 返回:
/// - 实际绘制在屏幕上的行数
pub(super) fn replay(
    stdout: &mut io::Stdout,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<usize> {
    let painted = replay_lines(stdout, viewport, lines)?;
    // 重绘区域之后可能残留旧行或旧 composer，一并清除（composer 随后由调用方重绘）
    let end_row = viewport
        .origin_row()
        .saturating_add(painted.min(usize::from(u16::MAX)) as u16);
    queue!(stdout, MoveTo(0, end_row), Clear(ClearType::FromCursorDown))?;
    stdout.flush()?;
    Ok(painted)
}

/// 清空可见屏与 scrollback 后从 source 全量重放（含 scrollback 前缀）。
///
/// 终端缩放会自行把可见行推入 scrollback 且不被记账，旧内容与重放内容
/// 会形成双份残留。此路径清空后按顺序重放整个窗口：超出可视预算的前缀
/// 通过真实滚动重新进入 scrollback，可视尾部留在屏幕上。
///
/// 参数:
/// - `output`: 终端输出句柄
/// - `viewport`: 已按新尺寸更新的 inline viewport（origin 必须为 0）
/// - `lines`: 当前宽度下的预换行 transcript 行
///
/// 返回:
/// - 实际留在屏幕上的行数
pub(super) fn replay_full<W: Write>(
    output: &mut W,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<usize> {
    let rows = usize::from(viewport.size().rows).max(1);
    let composer = usize::from(viewport.composer_height());
    let visible_budget = rows.saturating_sub(composer).max(1);
    // 绘制期间隐藏光标：中途 flush 时若光标可见，Windows 终端会在
    // 输出尾行与输入框之间来回闪动；最终位置由 composer 的 Show 恢复
    queue!(output, Hide)?;
    // 1. 清空可见屏与 scrollback，从顶部开始顺序输出
    queue!(
        output,
        MoveTo(0, 0),
        Clear(ClearType::All),
        Print(CLEAR_SCROLLBACK),
        Print(DISABLE_AUTOWRAP)
    )?;
    for line in lines {
        queue!(output, Print(line.as_str()), Print("\r\n"))?;
    }
    // 2. 计算内容尾行位置：打印 n 行后光标位于 min(n, rows-1)
    let total = lines.len();
    let content_tail = if total >= rows {
        // 已发生滚动，尾行固定在倒数第二行（最后的换行又滚了一行）
        rows.saturating_sub(2)
    } else {
        total.saturating_sub(1)
    };
    // 3. 内容顶到 composer 预算时，用底行换行把尾行滚动到 composer 上方
    let target_tail = total.min(visible_budget).saturating_sub(1);
    if total >= visible_budget && content_tail > target_tail {
        queue!(output, MoveTo(0, rows.saturating_sub(1) as u16))?;
        for _ in 0..content_tail - target_tail {
            queue!(output, Print("\r\n"))?;
        }
    }
    queue!(output, Print(ENABLE_AUTOWRAP))?;
    output.flush()?;
    Ok(total.min(visible_budget))
}

#[cfg(test)]
mod tests {
    use super::replay_full;
    use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};
    use crate::render::transcript::AnsiLine;

    /// 构造指定尺寸与 composer 高度的重锚 viewport。
    fn anchored_viewport(rows: u16, composer: u16, total: usize) -> InlineViewport {
        let size = TerminalSize { cols: 80, rows };
        let mut viewport = InlineViewport::new();
        viewport.restart_at(size, 0);
        viewport.update(size, composer, total);
        viewport
    }

    /// 生成 n 行测试内容。
    fn lines(n: usize) -> Vec<AnsiLine> {
        (0..n).map(|i| AnsiLine::new(format!("line-{i}"))).collect()
    }

    #[test]
    fn short_content_stays_fully_on_screen() {
        let viewport = anchored_viewport(24, 4, 10);
        let mut sink = Vec::new();
        let painted = replay_full(&mut sink, &viewport, &lines(10)).unwrap();
        let output = String::from_utf8(sink).unwrap();
        assert_eq!(painted, 10);
        // 清屏 + 清 scrollback + 从顶部输出
        assert!(output.contains("\x1b[2J"));
        assert!(output.contains("\x1b[3J"));
        assert!(output.contains("line-0"));
        assert!(output.contains("line-9"));
    }

    #[test]
    fn long_content_scrolls_prefix_into_scrollback() {
        // 24 行终端、4 行 composer：可视预算 20 行；60 行内容 → 40 行进 scrollback
        let viewport = anchored_viewport(24, 4, 60);
        let mut sink = Vec::new();
        let painted = replay_full(&mut sink, &viewport, &lines(60)).unwrap();
        let output = String::from_utf8(sink).unwrap();
        assert_eq!(painted, 20);
        // 所有行都被顺序输出（前缀经真实滚动进入 scrollback）
        assert!(output.contains("line-0"));
        assert!(output.contains("line-59"));
        // composer 空间通过底行换行腾出
        assert!(output.contains("\x1b[24;1H"));
    }

    #[test]
    fn empty_content_clears_without_scrolling() {
        let viewport = anchored_viewport(24, 4, 0);
        let mut sink = Vec::new();
        let painted = replay_full(&mut sink, &viewport, &[]).unwrap();
        assert_eq!(painted, 0);
        let output = String::from_utf8(sink).unwrap();
        assert!(output.contains("\x1b[3J"));
    }
}

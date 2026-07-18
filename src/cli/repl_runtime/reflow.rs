use super::history_insert::replay_lines;
use super::viewport::InlineViewport;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

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

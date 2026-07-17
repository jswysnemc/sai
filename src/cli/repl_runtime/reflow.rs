use super::history_insert::replay_lines;
use super::viewport::InlineViewport;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

/// 清除 Sai 受管区域并从 source 行重放历史。
///
/// 参数:
/// - `stdout`: 终端输出句柄
/// - `viewport`: 当前 inline viewport
/// - `lines`: 当前宽度下的预换行 transcript 行
///
/// 返回:
/// - 重放是否成功
pub(super) fn replay(
    stdout: &mut io::Stdout,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<()> {
    queue!(
        stdout,
        MoveTo(0, viewport.origin_row()),
        Clear(ClearType::FromCursorDown)
    )?;
    replay_lines(stdout, viewport, lines)?;
    stdout.flush()?;
    Ok(())
}

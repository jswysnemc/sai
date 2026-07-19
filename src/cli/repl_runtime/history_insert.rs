use super::viewport::InlineViewport;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::{self, Write};

/// 输出定位行期间关闭终端自动换行，防止测宽偏差引发滚动漂移。
const DISABLE_AUTOWRAP: &str = "\x1b[?7l";
/// 恢复终端自动换行。
const ENABLE_AUTOWRAP: &str = "\x1b[?7h";

/// 历史追加对终端 viewport 的影响。
pub(super) struct AppendOutcome {
    pub(super) scrolled_rows: u16,
}

/// 将完整 transcript 重绘到 composer viewport 上方。
///
/// 参数:
/// - `stdout`: 终端输出句柄
/// - `viewport`: 当前 inline viewport
/// - `lines`: 已按当前宽度预换行的历史行
///
/// 返回:
/// - 实际绘制的行数
pub(super) fn replay_lines(
    stdout: &mut io::Stdout,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<usize> {
    let painted = replay_lines_to(stdout, viewport, lines)?;
    stdout.flush()?;
    Ok(painted)
}

/// 执行一次增量协调：修补变化行、追加新行、清理收缩行。
///
/// 参数:
/// - `output`: 支持终端控制序列的目标输出
/// - `previous_viewport`: 本次同步前的 inline viewport
/// - `viewport`: 已按新行数更新的 inline viewport
/// - `patches`: 待重写的 `(全局行号, 新内容)` 列表
/// - `append`: 全局行号从旧总行数起的新增行
/// - `old_total`: 同步前的总行数
/// - `new_total`: 同步后的总行数
/// - `offscreen`: 已滚入 scrollback 的行数（其之前的行不可触碰）
///
/// 返回:
/// - 本次追加造成的终端滚动行数
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_delta<W: Write>(
    output: &mut W,
    previous_viewport: &InlineViewport,
    viewport: &InlineViewport,
    patches: &[(usize, AnsiLine)],
    append: &[AnsiLine],
    old_total: usize,
    new_total: usize,
    offscreen: usize,
) -> Result<AppendOutcome> {
    queue!(output, Print(DISABLE_AUTOWRAP))?;
    // 1. 修补仍在屏幕上的变化行（坐标基于旧 viewport，修补不移动其他行）
    for (row, line) in patches {
        let Some(screen_row) = screen_row_for(previous_viewport, old_total, *row, offscreen) else {
            continue;
        };
        queue!(
            output,
            MoveTo(0, screen_row),
            Clear(ClearType::CurrentLine),
            Print(line.as_str())
        )?;
    }
    // 2. 行数收缩时清掉新末尾之后的所有受管行（含旧 composer 区域，随后由 composer 重绘）
    if new_total < old_total {
        let clear_row = screen_row_for(previous_viewport, old_total, new_total, offscreen)
            .unwrap_or(previous_viewport.origin_row());
        queue!(
            output,
            MoveTo(0, clear_row),
            Clear(ClearType::FromCursorDown)
        )?;
    }
    // 3. 追加新行：屏幕未满时直接写入，满后用真实终端滚动保留原生 scrollback
    let outcome = append_lines_to(output, previous_viewport, viewport, append)?;
    queue!(output, Print(ENABLE_AUTOWRAP))?;
    output.flush()?;
    Ok(outcome)
}

/// 计算全局行号在屏幕上的行位置。
///
/// 参数:
/// - `viewport`: 行号对应时刻的 inline viewport
/// - `total`: 该时刻的总行数
/// - `row`: 全局行号
/// - `offscreen`: 已滚出行数
///
/// 返回:
/// - 行仍在屏幕受管区域内时返回屏幕行号
fn screen_row_for(
    viewport: &InlineViewport,
    total: usize,
    row: usize,
    offscreen: usize,
) -> Option<u16> {
    if row < offscreen || row > total {
        return None;
    }
    let from_bottom = total - row;
    let composer_top = usize::from(viewport.composer_top());
    let screen_row = composer_top.checked_sub(from_bottom)?;
    if screen_row < usize::from(viewport.origin_row()) {
        return None;
    }
    Some(screen_row.min(usize::from(u16::MAX)) as u16)
}

/// 将完整 transcript 按行定位到当前历史区域。
///
/// 参数:
/// - `output`: 支持终端控制序列的目标输出
/// - `viewport`: 当前 inline viewport
/// - `lines`: 已按当前宽度预换行的历史行
///
/// 返回:
/// - 实际绘制的行数
fn replay_lines_to<W: Write>(
    output: &mut W,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<usize> {
    let history_height = viewport.history_height();
    if history_height == 0 || lines.is_empty() {
        return Ok(0);
    }

    let visible_start = lines.len().saturating_sub(usize::from(history_height));
    queue!(output, Print(DISABLE_AUTOWRAP))?;
    let mut painted = 0usize;
    for (row, line) in lines[visible_start..].iter().enumerate() {
        queue!(
            output,
            MoveTo(0, viewport.origin_row().saturating_add(row as u16)),
            Clear(ClearType::CurrentLine),
            Print(line.as_str())
        )?;
        painted += 1;
    }
    queue!(output, Print(ENABLE_AUTOWRAP))?;
    Ok(painted)
}

/// 在 composer 上方追加稳定历史行。
///
/// 历史尚未填满终端时，直接写入旧 composer 顶部；填满后使用终端完整滚动区，
/// 使旧行进入原生 scrollback，而不是在受限 DECSTBM 区域内丢弃。
///
/// 参数:
/// - `output`: 支持终端控制序列的目标输出
/// - `previous_viewport`: 追加前的 inline viewport
/// - `viewport`: 当前 inline viewport
/// - `lines`: 已按当前宽度预换行的新增历史行
///
/// 返回:
/// - 写入是否成功
fn append_lines_to<W: Write>(
    output: &mut W,
    previous_viewport: &InlineViewport,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<AppendOutcome> {
    if lines.is_empty() {
        return Ok(AppendOutcome { scrolled_rows: 0 });
    }

    let previous_top = previous_viewport.composer_top();
    let composer_top = viewport.composer_top();
    let direct_rows = usize::from(composer_top.saturating_sub(previous_top));
    let direct_line_count = lines.len().min(direct_rows);
    for (index, line) in lines[..direct_line_count].iter().enumerate() {
        queue!(
            output,
            MoveTo(0, previous_top.saturating_add(index as u16)),
            Clear(ClearType::CurrentLine),
            Print(line.as_str())
        )?;
    }
    if direct_line_count == lines.len() {
        return Ok(AppendOutcome { scrolled_rows: 0 });
    }

    for line in &lines[direct_line_count..] {
        queue!(
            output,
            // 1. 在屏幕最后一行换行，交由终端把旧屏幕行写入原生 scrollback
            MoveTo(0, previous_viewport.size().rows.saturating_sub(1)),
            Print("\r\n"),
            // 2. 末条历史固定在 composer 紧上方，随后由 composer 重绘覆盖旧输入区
            MoveTo(0, composer_top.saturating_sub(1)),
            Clear(ClearType::CurrentLine),
            Print(line.as_str())
        )?;
    }
    Ok(AppendOutcome {
        scrolled_rows: lines
            .len()
            .saturating_sub(direct_line_count)
            .min(u16::MAX as usize) as u16,
    })
}

#[cfg(test)]
mod tests {
    use super::{append_lines_to, apply_delta, replay_lines_to, screen_row_for};
    use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};
    use crate::render::transcript::AnsiLine;

    /// 验证完整重放逐行定位，不追加导致首行滚出的换行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn replay_lines_do_not_scroll_first_history_row() {
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 3, 4);
        let mut output = Vec::new();
        let lines = vec![
            AnsiLine::new("first".to_string()),
            AnsiLine::new("second".to_string()),
        ];

        let painted = replay_lines_to(&mut output, &viewport, &lines).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[1;1H\x1b[2Kfirst\x1b[2;1H\x1b[2Ksecond"));
        assert!(!output.contains("\r\n"));
        assert_eq!(painted, 2);
    }

    /// 验证有背景的 diff 行不会在打印后被默认背景清行覆盖。
    #[test]
    fn replay_clears_before_printing_filled_diff_lines() {
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 3, 1);
        let mut output = Vec::new();
        let line = AnsiLine::new("\x1b[48;5;22mchanged\x1b[K\x1b[0m".to_string());

        replay_lines_to(&mut output, &viewport, &[line]).unwrap();

        let output = String::from_utf8(output).unwrap();
        let clear = output.find("\x1b[2K").unwrap();
        let background = output.find("\x1b[48;5;22m").unwrap();
        assert!(clear < background);
        assert!(!output[background..].contains("\x1b[2K"));
    }

    /// 验证历史未填满时，新行直接写入旧 composer 顶部。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn appended_lines_follow_short_history() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 3, 4);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 3, 5);
        let mut output = Vec::new();

        let outcome = append_lines_to(
            &mut output,
            &previous,
            &viewport,
            &[AnsiLine::new("next".to_string())],
        )
        .unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[5;1H\x1b[2Knext"));
        assert!(!output.contains("\x1b[r"));
        assert_eq!(outcome.scrolled_rows, 0);
    }

    /// 验证历史填满后使用完整终端滚动，保留原生 scrollback。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn appended_lines_scroll_the_full_terminal_after_history_fills() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 3, 80);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 3, 81);
        let mut output = Vec::new();

        let outcome = append_lines_to(
            &mut output,
            &previous,
            &viewport,
            &[AnsiLine::new("next".to_string())],
        )
        .unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[24;1H\r\n\x1b[21;1H\x1b[2Knext"));
        assert_eq!(outcome.scrolled_rows, 1);
    }

    /// 验证大块追加先填满空余历史行，再进入终端 scrollback。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn large_append_uses_scrollback_after_filling_open_history_rows() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 3, 4);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 3, 22);
        let lines = (0..18)
            .map(|index| AnsiLine::new(format!("line-{index}")))
            .collect::<Vec<_>>();
        let mut output = Vec::new();

        append_lines_to(&mut output, &previous, &viewport, &lines).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("\x1b[5;1H\x1b[2Kline-0"));
        assert!(output.contains("\x1b[21;1H\x1b[2Kline-16"));
        assert!(output.contains("\x1b[24;1H\r\n\x1b[21;1H\x1b[2Kline-17"));
    }

    /// 验证修补只重写目标行，其他行不受影响。
    #[test]
    fn patch_rewrites_only_target_row() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 0, 5);
        let viewport = previous;
        let mut sink = Vec::new();

        // 5 行历史（全局行 0..5），修补行 3
        let patches = vec![(3usize, AnsiLine::new("patched".to_string()))];
        let outcome = apply_delta(&mut sink, &previous, &viewport, &patches, &[], 5, 5, 0).unwrap();

        let output = String::from_utf8(sink).unwrap();
        // composer_top = 5，行 3 距底部 2 行 → 屏幕第 4 行（1 起）
        assert!(output.contains("\x1b[4;1H\x1b[2Kpatched"));
        assert!(!output.contains("\x1b[5;1H"));
        assert_eq!(outcome.scrolled_rows, 0);
    }

    /// 验证收缩会从新末尾清到屏幕底部。
    #[test]
    fn shrink_clears_rows_below_new_tail() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 0, 6);
        let mut viewport = InlineViewport::new();
        viewport.update(TerminalSize { cols: 80, rows: 24 }, 0, 4);
        let mut sink = Vec::new();

        apply_delta(&mut sink, &previous, &viewport, &[], &[], 6, 4, 0).unwrap();

        let output = String::from_utf8(sink).unwrap();
        // 全局行 4 位于屏幕第 5 行（1 起），从那里清到屏幕底
        assert!(output.contains("\x1b[5;1H\x1b[J"));
    }

    /// 验证已滚出屏幕的行不会被修补。
    #[test]
    fn patch_skips_rows_in_scrollback() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 0, 24);
        assert_eq!(
            screen_row_for(&previous, 100, 75, 76),
            None,
            "已滚出行必须跳过"
        );
        assert_eq!(screen_row_for(&previous, 100, 80, 76), Some(4));
    }

    /// 验证 apply_delta 在输出前后关闭并恢复自动换行。
    #[test]
    fn delta_disables_autowrap_during_positioned_writes() {
        let mut previous = InlineViewport::new();
        previous.update(TerminalSize { cols: 80, rows: 24 }, 0, 2);
        let viewport = previous;
        let mut sink = Vec::new();

        apply_delta(
            &mut sink,
            &previous,
            &viewport,
            &[(1usize, AnsiLine::new("x".to_string()))],
            &[],
            2,
            2,
            0,
        )
        .unwrap();

        let output = String::from_utf8(sink).unwrap();
        let disable = output.find("\x1b[?7l").unwrap();
        let enable = output.find("\x1b[?7h").unwrap();
        assert!(disable < enable);
    }
}

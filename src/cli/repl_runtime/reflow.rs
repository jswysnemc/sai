use super::history_insert::replay_lines;
use super::viewport::InlineViewport;
use crate::render::transcript::AnsiLine;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::Write;

/// 输出期间关闭终端自动换行。
const DISABLE_AUTOWRAP: &str = "\x1b[?7l";
/// 恢复终端自动换行。
const ENABLE_AUTOWRAP: &str = "\x1b[?7h";

/// 从 source 行重放历史可视尾部，并清理其后的旧内容。
///
/// 先逐行重绘（每行独立清行），再从最后一行之后清到屏幕底部，
/// 避免整屏先清空再绘制造成的闪烁。输出写入调用方的帧缓冲。
///
/// 参数:
/// - `output`: 当前帧的输出缓冲
/// - `viewport`: 当前 inline viewport
/// - `lines`: 当前宽度下的预换行 transcript 行
///
/// 返回:
/// - 实际绘制在屏幕上的行数
pub(super) fn replay<W: Write>(
    output: &mut W,
    viewport: &InlineViewport,
    lines: &[AnsiLine],
) -> Result<usize> {
    let painted = replay_lines(output, viewport, lines)?;
    // 重绘区域之后可能残留旧行或旧 composer，一并清除（composer 随后由调用方重绘）
    let end_row = viewport
        .origin_row()
        .saturating_add(painted.min(usize::from(u16::MAX)) as u16);
    queue!(output, MoveTo(0, end_row), Clear(ClearType::FromCursorDown))?;
    Ok(painted)
}

/// 清屏后从 source 重放可视历史（不清 scrollback）。
///
/// 终端缩放会自行把可见行推入 scrollback 且不被记账，若不清可见屏会与
/// 重放内容形成双份残留，因此清空可见屏后按顺序重放整个窗口：超出可视
/// 预算的前缀经真实滚动进入 scrollback，可视尾部留在屏幕上。
///
/// 只清可见屏、不发 ED 3：ED 3 会连终端原生回滚一起抹掉，其中可能包含
/// 启动 sai 之前的用户历史，一次横向缩放就永久丢失。历史重建改为追加，
/// 因此调用方会把重放行数量控制在可视预算附近而非整个 row cap。
///
/// 参数:
/// - `output`: 当前帧的输出缓冲
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
    // 绘制期间隐藏光标：光标会随整屏重放扫过全部内容行，
    // 可见状态下表现为一次跳动；最终位置由 composer 的 Show 恢复
    queue!(output, Hide)?;
    // 1. 只清空可见屏后从顶部开始顺序输出，保留终端 scrollback。
    //    ED 序列不影响 Kitty 图像放置，必须显式发图形删除命令，
    //    否则重放后旧图残留在屏幕上
    queue!(
        output,
        MoveTo(0, 0),
        Clear(ClearType::All),
        Print(crate::render::terminal_image::KITTY_DELETE_PLACEMENTS),
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
        // 清可见屏 + 从顶部输出，但不发 ED 3（保留终端原生回滚）
        assert!(output.contains("\x1b[2J"));
        assert!(!output.contains("\x1b[3J"));
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
        assert!(output.contains("\x1b[2J"));
    }

    /// 重放只清可见屏：ED 3 会连启动前的终端历史一起抹掉，不能出现。
    #[test]
    fn replay_never_destroys_native_scrollback() {
        let viewport = anchored_viewport(24, 4, 60);
        let mut sink = Vec::new();
        replay_full(&mut sink, &viewport, &lines(60)).unwrap();
        let output = String::from_utf8(sink).unwrap();
        assert!(!output.contains("\x1b[3J"));
    }
}

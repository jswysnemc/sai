use super::draft::{Draft, Outcome};
use super::view::{self, PreviewContext};
use crate::config_tui::{theme, ui};
use crate::i18n::text as t;
use crate::state::CompactionBudgetPolicy;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, Write};

/// 内联面板固定行数：标题、正文窗口和底栏。
const FRAME_ROWS: u16 = 16;

/// 【上下文】【内联设置】在当前画面底部打开压缩策略，不进入备用屏。
///
/// 参数:
/// - `context`: 窗口与作用范围
/// - `policy`: 当前策略
/// - `defaults`: 恢复目标
///
/// 返回:
/// - 保存、恢复或取消
pub(super) fn run(
    context: &PreviewContext,
    policy: CompactionBudgetPolicy,
    defaults: CompactionBudgetPolicy,
) -> Result<Outcome> {
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        terminal::enable_raw_mode()?;
    }
    struct ModeGuard {
        was_raw: bool,
    }
    impl Drop for ModeGuard {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), Show);
            if !self.was_raw {
                let _ = terminal::disable_raw_mode();
            }
        }
    }
    let _guard = ModeGuard { was_raw };

    let mut draft = Draft::new(policy, defaults);
    let mut stdout = io::stdout();
    let frame_rows = frame_rows();
    execute!(stdout, Hide)?;
    reserve_frame_space(frame_rows)?;
    let (_, cursor_y) = crossterm::cursor::position().unwrap_or((0, frame_rows.saturating_sub(1)));
    let anchor_y = cursor_y.saturating_sub(frame_rows.saturating_sub(1));

    let outcome = loop {
        let lines = frame_lines(&draft, context, frame_rows);
        if let Err(error) = draw_at(&mut stdout, anchor_y, frame_rows, &lines) {
            let _ = clear_frame(&mut stdout, anchor_y, frame_rows);
            return Err(error);
        }
        match crate::config_tui::input::read_key_event_with_timeout(None) {
            Ok(Some(key)) => {
                if let Some(outcome) = draft.handle(key.code) {
                    break outcome;
                }
            }
            Ok(None) => {}
            Err(error) => {
                let _ = clear_frame(&mut stdout, anchor_y, frame_rows);
                return Err(error);
            }
        }
    };
    clear_frame(&mut stdout, anchor_y, frame_rows)?;
    Ok(outcome)
}

/// 按终端高度收缩固定行数。
///
/// 返回:
/// - 实际占用行数，至少 1 行
fn frame_rows() -> u16 {
    let rows = terminal::size().map(|(_, rows)| rows).unwrap_or(FRAME_ROWS);
    FRAME_ROWS.min(rows).max(1)
}

/// 生成固定高度的内联文本。
///
/// 参数:
/// - `draft`: 编辑草稿
/// - `context`: 预览信息
/// - `rows`: 面板行数
///
/// 返回:
/// - 与行数等长的文本
fn frame_lines(draft: &Draft, context: &PreviewContext, rows: u16) -> Vec<String> {
    let cols = terminal::size().map(|(cols, _)| cols).unwrap_or(80);
    let width = cols.saturating_sub(1) as usize;
    let (body, selected) = view::content(draft, context, width.max(1));
    let body_rows = (rows as usize).saturating_sub(2);
    let start = if body.len() <= body_rows {
        0
    } else {
        selected
            .saturating_sub(body_rows.saturating_sub(1))
            .min(body.len() - body_rows)
    };
    let mut lines = vec![format!(
        "{}{}{}",
        theme::ACCENT,
        t("Compaction", "压缩策略"),
        theme::RESET
    )];
    for line in body.iter().skip(start).take(body_rows) {
        let style = if lines.len() - 1 + start == selected {
            theme::ACCENT
        } else {
            theme::VALUE
        };
        lines.push(format!(
            "{style}{}{}",
            ui::truncate(line, width),
            theme::RESET
        ));
    }
    while lines.len() + 1 < rows as usize {
        lines.push(String::new());
    }
    lines.push(theme::help_line(&[
        ("↑↓", t("select", "选择")),
        ("Enter", t("edit", "编辑")),
        ("s", t("save", "保存")),
        ("Esc", t("cancel", "取消")),
    ]));
    lines.truncate(rows as usize);
    lines
}

/// 预留下拉区域。
///
/// 参数:
/// - `rows`: 占用行数
///
/// 返回:
/// - 是否成功
fn reserve_frame_space(rows: u16) -> Result<()> {
    let mut stdout = io::stdout();
    for _ in 1..rows {
        queue!(stdout, crossterm::style::Print("\r\n"))?;
    }
    stdout.flush()?;
    Ok(())
}

/// 在固定锚点绘制一帧。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 首行行号
/// - `frame_rows`: 预留总行数
/// - `lines`: 待绘制内容
///
/// 返回:
/// - 绘制结果
fn draw_at(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    lines: &[String],
) -> Result<()> {
    ui::begin_synced_frame(stdout)?;
    for row in 0..frame_rows {
        queue!(
            stdout,
            MoveTo(0, anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine)
        )?;
        if let Some(line) = lines.get(row as usize) {
            queue!(stdout, crossterm::style::Print(line))?;
        }
    }
    ui::end_synced_frame(stdout)?;
    Ok(())
}

/// 清空内联区域。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 首行行号
/// - `frame_rows`: 预留总行数
///
/// 返回:
/// - 是否成功
fn clear_frame(stdout: &mut io::Stdout, anchor_y: u16, frame_rows: u16) -> Result<()> {
    ui::begin_synced_frame(stdout)?;
    for row in 0..frame_rows {
        queue!(
            stdout,
            MoveTo(0, anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(stdout, MoveTo(0, anchor_y))?;
    ui::end_synced_frame(stdout)?;
    Ok(())
}

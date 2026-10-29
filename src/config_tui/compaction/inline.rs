//! 压缩策略面板接替输入区，空闲时不输出，结束时恢复调用方画面。

use super::{
    card,
    draft::{Draft, Outcome},
    view::{self, PreviewContext},
};
use crate::{question_tui::OverlayPlacement, state::CompactionBudgetPolicy};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    execute, terminal,
};
use std::{io, time::Duration};

struct ModeGuard(bool);

impl Drop for ModeGuard {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show);
        if !self.0 {
            let _ = terminal::disable_raw_mode();
        }
    }
}

/// 【上下文】【内联设置】仅在草稿或尺寸变化时绘制，保存前不写入任何配置。
/// 参数：上下文、当前/默认策略与区域分配回调；返回：保存、恢复或取消动作。
pub(super) fn run(
    context: &PreviewContext,
    policy: CompactionBudgetPolicy,
    defaults: CompactionBudgetPolicy,
    mut place: impl FnMut(u16) -> Result<OverlayPlacement>,
) -> Result<Outcome> {
    let was_raw = terminal::is_raw_mode_enabled()?;
    if !was_raw {
        terminal::enable_raw_mode()?;
    }
    let _guard = ModeGuard(was_raw);
    let mut stdout = io::stdout();
    execute!(stdout, Hide)?;
    let mut draft = Draft::new(policy, defaults);
    let mut previous = Vec::new();
    let mut geometry = None;
    let mut top = 0;
    loop {
        let (cols, rows) = terminal::size()?;
        let lines = card::fit_height(
            view::frame(&draft, context, usize::from(cols)),
            usize::from(rows.saturating_sub(1).max(1)),
        );
        let shape = (cols, rows, lines.len());
        if geometry != Some(shape) {
            top = place(lines.len() as u16)?.anchor_y;
            execute!(stdout, Hide)?;
            geometry = Some(shape);
            previous.clear();
        }
        if lines != previous {
            crate::config_tui::ui::begin_synced_frame(&mut stdout)?;
            crate::render::terminal_rows::paint_changed_rows(
                &mut stdout,
                top,
                usize::from(cols),
                &lines,
                Some(&previous),
            )?;
            crate::config_tui::ui::end_synced_frame(&mut stdout)?;
            previous = lines;
        }
        if let Some(key) =
            crate::config_tui::input::read_key_event_with_timeout(Some(Duration::from_millis(100)))?
        {
            if let Some(outcome) = draft.handle_event(key) {
                return Ok(outcome);
            }
        }
    }
}

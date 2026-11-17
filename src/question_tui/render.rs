use super::session::QuestionSession;
use super::state::QuestionState;
use crate::question::QuestionRequest;
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    queue, terminal,
};
use std::io::Write;

/// 【终端提问】【差异绘制】生成页面帧并仅向终端写入变化行，同步更新鼠标命中几何与编辑光标。
///
/// 参数:
/// - `session`: 提问终端会话
/// - `request`: 问题集合
/// - `state`: 提问交互状态
///
/// 返回:
/// - 终端写入结果
pub(super) fn draw(
    session: &mut QuestionSession,
    request: &QuestionRequest,
    state: &mut QuestionState,
) -> Result<()> {
    let (cols, _) = terminal::size().unwrap_or((80, 24));
    let view = super::view::compose(
        request,
        state,
        usize::from(cols),
        usize::from(session.panel_lines),
    );
    session.hits = view.hits;
    if session.previous != view.lines {
        queue!(session.stdout, Hide)?;
        crate::render::terminal_rows::paint_changed_rows(
            &mut session.stdout,
            session.anchor_y,
            usize::from(cols),
            &view.lines,
            Some(&session.previous),
        )?;
        session.previous = view.lines;
    }
    if let Some((col, row)) = view.cursor {
        queue!(
            session.stdout,
            MoveTo(col as u16, session.anchor_y.saturating_add(row as u16)),
            Show
        )?;
    } else {
        queue!(session.stdout, Hide)?;
    }
    session.stdout.flush()?;
    Ok(())
}

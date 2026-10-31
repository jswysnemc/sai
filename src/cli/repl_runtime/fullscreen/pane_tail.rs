//! 固定右栏延伸到输入区旁边，覆盖旧背景残留但不改写左侧输入。

use super::git_preview_view::{panel_rows, PreviewLayout};
use super::layout::FullscreenLayout;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_style::clip;
use crate::cli::repl_runtime::ReplRuntime;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo};
use crossterm::queue;
use crossterm::terminal::{DisableLineWrap, EnableLineWrap};
use std::io::Write;

#[derive(PartialEq, Eq)]
pub(super) struct PaneTail {
    left: u16,
    top: u16,
    rows: Vec<String>,
}

impl ReplRuntime {
    /// 【终端侧栏】【底部右栏】概览与审查均延伸到屏幕底部，与左侧输入分别绘制。
    /// 参数: force 为输入框是否重绘过同一行；返回: 是否写入了右栏内容。
    pub(in crate::cli::repl_runtime) fn paint_pane_tail(&mut self, force: bool) -> Result<bool> {
        let Some(session) = self.fullscreen.as_mut() else {
            return Ok(false);
        };
        let Some(layout) = session
            .state
            .layout
            .filter(|layout| layout.sidebar.is_some())
        else {
            return Ok(false);
        };
        let geometry = PreviewLayout::compute(&layout, session.sidebar.git.summary.files.len());
        let width = geometry.width.saturating_sub(3);
        let palette = SidebarPalette::current();
        let panel = if layout.review {
            panel_rows(
                &session.sidebar.preview,
                &session.sidebar.git.summary,
                &layout,
            )
        } else {
            super::sidebar_view::panel_rows(&session.sidebar, &layout, &self.sidebar_control)
        };
        let mut rows = panel
            .into_iter()
            .skip(usize::from(layout.composer_top))
            .map(|line| {
                let text = clip(&line, width);
                let padding = width.saturating_sub(crate::cli::repl_text::visible_width(&text));
                format!(
                    "\x1b[0m{}│\x1b[0m {text}{} ",
                    fg(palette.border),
                    " ".repeat(padding)
                )
            })
            .collect::<Vec<_>>();
        super::sidebar_resize::decorate_divider(&self.sidebar_control, &mut rows);
        let next = PaneTail {
            left: layout.main_width,
            top: layout.composer_top,
            rows,
        };
        if !force && session.pane_tail.as_ref() == Some(&next) {
            return Ok(false);
        }
        // 1. 【终端审查】【定列绘制】关闭自动换行，避免末行末列引起整个终端滚动
        queue!(self.frame, Hide, DisableLineWrap)?;
        for (index, text) in next.rows.iter().enumerate() {
            queue!(self.frame, MoveTo(next.left, next.top + index as u16))?;
            write!(self.frame, "{text}")?;
        }
        queue!(self.frame, EnableLineWrap)?;
        session.pane_tail = Some(next);
        Ok(true)
    }

    /// 【终端侧栏】【布局失效】移动边界或关闭面板时丢弃旧基线，随后同帧覆盖，不先清空造成闪烁。
    /// 参数: layout 为目标布局；返回: 无。
    pub(super) fn invalidate_pane_tail(&mut self, layout: &FullscreenLayout) {
        let Some(session) = self.fullscreen.as_mut() else {
            return;
        };
        if session.pane_tail.as_ref().is_some_and(|previous| {
            layout.sidebar.is_some()
                && previous.left == layout.main_width
                && previous.top == layout.composer_top
        }) {
            return;
        }
        let Some(_) = session.pane_tail.take() else {
            return;
        };
        session.state.previous = None;
        self.last_composer_signature = None;
    }
}

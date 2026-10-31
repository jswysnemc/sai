//! 输入框右侧的固定审查区域，只绘制预留列，不覆盖左侧输入。

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
pub(super) struct ReviewTail {
    left: u16,
    top: u16,
    rows: Vec<String>,
}

impl ReplRuntime {
    /// 【终端审查】【底部右栏】审查区延伸到屏幕底部，与左侧输入分别绘制。
    /// 参数: force 为输入框是否重绘过同一行；返回: 是否写入了审查内容。
    pub(in crate::cli::repl_runtime) fn paint_review_tail(&mut self, force: bool) -> Result<bool> {
        let Some(session) = self.fullscreen.as_mut() else {
            return Ok(false);
        };
        let Some(layout) = session.state.layout.filter(|layout| layout.review) else {
            return Ok(false);
        };
        if session.sidebar.preview.path.is_none() {
            return Ok(false);
        }
        let geometry = PreviewLayout::compute(&layout, session.sidebar.git.summary.files.len());
        let width = geometry.width.saturating_sub(3);
        let palette = SidebarPalette::current();
        let panel = panel_rows(
            &session.sidebar.preview,
            &session.sidebar.git.summary,
            &layout,
        );
        let rows = panel
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
            .collect();
        let next = ReviewTail {
            left: layout.main_width,
            top: layout.composer_top,
            rows,
        };
        if !force && session.review_tail.as_ref() == Some(&next) {
            return Ok(false);
        }
        // 1. 【终端审查】【定列绘制】关闭自动换行，避免末行末列引起整个终端滚动
        queue!(self.frame, Hide, DisableLineWrap)?;
        for (index, text) in next.rows.iter().enumerate() {
            queue!(self.frame, MoveTo(next.left, next.top + index as u16))?;
            write!(self.frame, "{text}")?;
        }
        queue!(self.frame, EnableLineWrap)?;
        session.review_tail = Some(next);
        Ok(true)
    }

    /// 【终端审查】【退出清理】恢复普通布局前清理旧右栏，随后由聊天和输入重新绘制。
    /// 参数: layout 为目标布局；返回: 清理结果。
    pub(super) fn clear_review_tail(&mut self, layout: &FullscreenLayout) -> Result<()> {
        if layout.review {
            return Ok(());
        }
        let Some(session) = self.fullscreen.as_mut() else {
            return Ok(());
        };
        let Some(previous) = session.review_tail.take() else {
            return Ok(());
        };
        let width = previous
            .rows
            .first()
            .map_or(0, |line| crate::cli::repl_text::visible_width(line))
            .min(usize::from(layout.cols.saturating_sub(previous.left)));
        queue!(self.frame, Hide, DisableLineWrap)?;
        if width > 0 {
            for index in 0..previous.rows.len() {
                let row = previous.top + index as u16;
                if row >= layout.rows {
                    break;
                }
                queue!(self.frame, MoveTo(previous.left, row))?;
                write!(self.frame, "\x1b[0m{}", " ".repeat(width))?;
            }
        }
        queue!(self.frame, EnableLineWrap)?;
        session.state.previous = None;
        self.last_composer_signature = None;
        Ok(())
    }
}

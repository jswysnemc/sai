use super::mouse::QuestionHitMap;
use super::overlay::{OverlayPlacement, MAX_PANEL_LINES};
use super::summary::answered_card_lines;
use crate::i18n::text as t;
use crate::question::{QuestionAnswers, QuestionRequest};
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use std::io::{self, Write};

const BAR: &str = " ";

pub(super) struct QuestionSession {
    pub(super) _mouse: Option<crate::cli::mouse_capture::MouseCapture>,
    pub(super) stdout: io::Stdout,
    pub(super) anchor_y: u16,
    pub(super) panel_lines: u16,
    pub(super) previous: Vec<String>,
    pub(super) hits: QuestionHitMap,
    /// 是否由本会话启用 raw mode 与括号粘贴
    pub(super) owns_terminal: bool,
    /// 是否为底部覆盖层；结束时只清覆盖区，不写摘要卡片
    pub(super) overlay: bool,
    /// 内联面板保留正文后的锚点；全屏面板随窗口底部移动
    pub(super) inline_anchor: Option<u16>,
}

impl QuestionSession {
    /// 【终端提问】【启动会话】启动原始模式终端会话、获取鼠标捕获并记录面板锚点。
    ///
    /// 参数:
    /// - `panel_lines`: 面板占用行数
    ///
    /// 返回:
    /// - 初始化完成的终端会话
    pub(super) fn start(panel_lines: u16) -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnableBracketedPaste, Hide) {
            let _ = execute!(stdout, DisableBracketedPaste, Show);
            let _ = terminal::disable_raw_mode();
            return Err(err.into());
        }
        let mouse = crate::cli::mouse_capture::MouseCapture::acquire().ok();
        let (_, cursor_y) =
            crossterm::cursor::position().unwrap_or((0, panel_lines.saturating_sub(1)));
        let anchor_y = cursor_y.saturating_sub(panel_lines.saturating_sub(1));
        Ok(Self {
            _mouse: mouse,
            stdout,
            anchor_y,
            panel_lines,
            previous: Vec::new(),
            hits: QuestionHitMap::default(),
            owns_terminal: true,
            overlay: false,
            inline_anchor: None,
        })
    }

    /// 【终端提问】【底部覆盖】在不开关外层 raw mode 的情况下启动覆盖会话并获取鼠标捕获。
    ///
    /// 参数:
    /// - `placement`: 覆盖区域布局
    ///
    /// 返回:
    /// - 覆盖层终端会话
    pub(super) fn start_overlay(placement: OverlayPlacement) -> Result<Self> {
        let mut stdout = io::stdout();
        execute!(stdout, Hide)?;
        let mouse = crate::cli::mouse_capture::MouseCapture::acquire().ok();
        Ok(Self {
            _mouse: mouse,
            stdout,
            anchor_y: placement.anchor_y,
            panel_lines: placement.panel_lines,
            previous: Vec::new(),
            hits: QuestionHitMap::default(),
            owns_terminal: false,
            overlay: true,
            inline_anchor: placement.inline_anchor,
        })
    }

    /// 【终端提问】【完成摘要】清理交互面板并在终端写入紧凑回答卡片。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    /// - `answers`: 已提交答案
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    pub(super) fn finish_answered(
        &mut self,
        request: &QuestionRequest,
        answers: &QuestionAnswers,
    ) -> Result<()> {
        if self.overlay {
            return self.finish_overlay();
        }
        self.clear()?;
        let width = terminal::size().map(|(cols, _)| cols).unwrap_or(80) as usize;
        let lines = answered_card_lines(request, answers, width, usize::from(self.panel_lines));
        crate::render::terminal_rows::paint_changed_rows(
            &mut self.stdout,
            self.anchor_y,
            width,
            &lines,
            None,
        )?;
        let row = self.anchor_y.saturating_add(
            lines
                .len()
                .min(usize::from(self.panel_lines.saturating_sub(1))) as u16,
        );
        queue!(
            self.stdout,
            crossterm::style::Print("\x1b[0m"),
            MoveTo(0, row),
            crossterm::style::Print("\r\n"),
            Show
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 【终端提问】【取消收尾】清理交互面板并输出取消状态。
    ///
    /// 参数:
    /// - `self`: 当前会话
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    pub(super) fn finish_cancelled(&mut self) -> Result<()> {
        if self.overlay {
            return self.finish_overlay();
        }
        self.clear()?;
        queue!(
            self.stdout,
            MoveTo(0, self.anchor_y),
            crossterm::style::Print(format!(
                "{BAR} {}{}\x1b[0m",
                crate::render::panel_palette::fg(
                    crate::render::panel_palette::PanelPalette::current().muted
                ),
                t("Question cancelled", "已取消提问")
            )),
            MoveTo(0, self.anchor_y.saturating_add(1)),
            Clear(ClearType::CurrentLine),
            Show
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 【终端提问】【清理覆盖层】仅擦除底部覆盖区并恢复光标可见。
    ///
    /// 参数:
    /// - `self`: 当前会话
    ///
    /// 返回:
    /// - 清理结果
    fn finish_overlay(&mut self) -> Result<()> {
        self.clear()?;
        queue!(self.stdout, Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 【终端提问】【面板清行】清空当前面板占用的全部终端行。
    ///
    /// 参数:
    /// - `self`: 当前会话
    ///
    /// 返回:
    /// - 清理成功时返回空结果
    pub(super) fn clear(&mut self) -> Result<()> {
        queue!(self.stdout, crossterm::style::Print("\x1b[0m"))?;
        for row in 0..self.panel_lines {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row)),
                Clear(ClearType::CurrentLine)
            )?;
        }
        Ok(())
    }

    /// 【终端提问】【窗口缩放】根据终端新尺寸调整面板高度和锚点。
    ///
    /// 参数:
    /// - `cols`: 终端总列数
    /// - `rows`: 终端总行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn resize_to_terminal(&mut self, cols: u16, rows: u16) {
        self.previous.clear();
        self.hits = QuestionHitMap::default();
        if self.overlay {
            // 1. 【终端提问】【底部覆盖】先清旧区，再按新尺寸贴底或保持内联锚点
            if !cfg!(test) {
                let _ = self.clear();
            }
            let placement = match self.inline_anchor {
                Some(anchor) => OverlayPlacement::inline(cols, rows, anchor),
                None => OverlayPlacement::bottom(cols, rows),
            };
            self.anchor_y = placement.anchor_y;
            self.panel_lines = placement.panel_lines;
            return;
        }
        self.panel_lines = rows.saturating_sub(1).clamp(1, MAX_PANEL_LINES);
        self.anchor_y = self.anchor_y.min(rows.saturating_sub(self.panel_lines));
    }
}

impl Drop for QuestionSession {
    fn drop(&mut self) {
        self._mouse.take();
        if self.owns_terminal {
            let _ = execute!(self.stdout, DisableBracketedPaste, Show);
            let _ = terminal::disable_raw_mode();
            return;
        }
        let _ = execute!(self.stdout, Show);
    }
}

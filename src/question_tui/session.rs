use super::*;

pub(super) struct QuestionSession {
    pub(super) stdout: io::Stdout,
    pub(super) anchor_y: u16,
    pub(super) panel_lines: u16,
    pub(super) previous: Vec<String>,
}

impl QuestionSession {
    /// 启动原始模式终端会话并记录面板锚点。
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
        let (_, cursor_y) =
            crossterm::cursor::position().unwrap_or((0, panel_lines.saturating_sub(1)));
        let anchor_y = cursor_y.saturating_sub(panel_lines.saturating_sub(1));
        Ok(Self {
            stdout,
            anchor_y,
            panel_lines,
            previous: Vec::new(),
        })
    }

    /// 清理交互面板并输出回答摘要。
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
        self.clear()?;
        let width = terminal::size().map(|(cols, _)| cols).unwrap_or(80) as usize;
        let content_width = width.saturating_sub(3).max(1);
        let keeps_blank_line = self.panel_lines > 1;
        let content_rows = self
            .panel_lines
            .saturating_sub(u16::from(keeps_blank_line))
            .max(1);
        let mut row = 0u16;
        for line in answered_summary_lines(request, answers, content_rows as usize) {
            self.write_answered_line(row, &line, content_width)?;
            row += 1;
        }
        if keeps_blank_line {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row)),
                Clear(ClearType::CurrentLine),
                crossterm::style::Print("\r\n")
            )?;
        } else {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row.saturating_sub(1))),
                crossterm::style::Print("\r\n")
            )?;
        }
        queue!(self.stdout, Clear(ClearType::CurrentLine), Show)?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 清理交互面板并输出取消状态。
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    pub(super) fn finish_cancelled(&mut self) -> Result<()> {
        self.clear()?;
        queue!(
            self.stdout,
            MoveTo(0, self.anchor_y),
            crossterm::style::Print(format!(
                "{BAR} \x1b[2m{}\x1b[0m",
                t("Question cancelled", "已取消提问")
            )),
            MoveTo(0, self.anchor_y.saturating_add(1)),
            Clear(ClearType::CurrentLine),
            Show
        )?;
        self.stdout.flush()?;
        Ok(())
    }

    /// 在指定面板行输出一条回答摘要。
    ///
    /// 参数:
    /// - `row`: 相对面板行号
    /// - `text`: 摘要文本
    /// - `width`: 最大显示宽度
    ///
    /// 返回:
    /// - 输出成功时返回空结果
    pub(super) fn write_answered_line(&mut self, row: u16, text: &str, width: usize) -> Result<()> {
        queue!(
            self.stdout,
            MoveTo(0, self.anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine),
            crossterm::style::Print(ANSWERED_BAR),
            crossterm::style::Print(" \x1b[2m\x1b[90m"),
            crossterm::style::Print(truncate_width(text, width)),
            crossterm::style::Print("\x1b[0m")
        )?;
        Ok(())
    }

    /// 清空当前面板占用的全部终端行。
    ///
    /// 返回:
    /// - 清理成功时返回空结果
    pub(super) fn clear(&mut self) -> Result<()> {
        for row in 0..self.panel_lines {
            queue!(
                self.stdout,
                MoveTo(0, self.anchor_y.saturating_add(row)),
                Clear(ClearType::CurrentLine)
            )?;
        }
        Ok(())
    }

    /// 根据终端新高度调整面板尺寸和锚点。
    ///
    /// 参数:
    /// - `rows`: 终端总行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn resize_to_terminal(&mut self, rows: u16) {
        self.previous.clear();
        self.panel_lines = rows.saturating_sub(1).clamp(1, MAX_PANEL_LINES);
        self.anchor_y = self.anchor_y.min(rows.saturating_sub(self.panel_lines));
    }
}

impl Drop for QuestionSession {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, DisableBracketedPaste, Show);
        let _ = terminal::disable_raw_mode();
    }
}

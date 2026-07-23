use super::*;

impl StreamRenderer {
    /// 切换当前流式输出模式。
    ///
    /// 参数:
    /// - `mode`: 新输出模式
    ///
    /// 返回:
    /// - 切换是否成功
    pub(super) fn switch_mode(&mut self, mode: ChatStreamKind) -> Result<()> {
        let mut stdout = io::stdout();
        match mode {
            ChatStreamKind::Reasoning => {
                // Full 模式在 flush 时统一输出折叠块；Summary 由 summary 负责
                if self.mode.is_some() && self.reasoning_mode != ReasoningDisplayMode::Full {
                    writeln!(stdout)?;
                }
                if self.reasoning_mode != ReasoningDisplayMode::Full {
                    execute!(stdout, SetForegroundColor(Color::DarkCyan))?;
                    writeln!(
                        stdout,
                        "{TOOL_BULLET} {}",
                        self.work_status
                            .unwrap_or(WorkStatus::Thinking)
                            .localized_label()
                    )?;
                }
            }
            ChatStreamKind::Content => {
                if self.mode == Some(ChatStreamKind::Reasoning)
                    && self.reasoning_mode != ReasoningDisplayMode::Full
                {
                    execute!(stdout, ResetColor)?;
                    writeln!(stdout)?;
                }
            }
        }
        stdout.flush()?;
        self.mode = Some(mode);
        Ok(())
    }

    /// 结束当前活动流式行。
    ///
    /// 返回:
    /// - 结束是否成功
    pub(super) fn end_active_stream_line(&mut self) -> Result<()> {
        self.finish_live_tool_status()?;
        if self.reasoning_mode == ReasoningDisplayMode::Summary
            && self.mode == Some(ChatStreamKind::Reasoning)
        {
            self.mode = None;
            return Ok(());
        }
        // Full 思考块在此折叠输出
        if self.mode == Some(ChatStreamKind::Reasoning)
            && self.reasoning_mode == ReasoningDisplayMode::Full
        {
            self.flush_full_reasoning_block()?;
            return Ok(());
        }
        if self.mode == Some(ChatStreamKind::Reasoning) {
            execute!(io::stdout(), ResetColor)?;
        } else if self.mode == Some(ChatStreamKind::Content) && !self.plain {
            let mut stdout = io::stdout();
            write!(stdout, "{}", self.markdown.flush())?;
            stdout.flush()?;
        }
        if self.mode.is_some() {
            println!();
            self.mode = None;
        }
        Ok(())
    }

    /// 将 Full 模式缓存的思考正文折叠输出到终端。
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn flush_full_reasoning_block(&mut self) -> Result<()> {
        if self.reasoning_full_buffer.trim().is_empty() {
            self.mode = None;
            self.reasoning_started = None;
            let _ = self.summary.clear_live_lines();
            return Ok(());
        }
        // 1. 先擦除 live 摘要行，再输出折叠正文
        let _ = self.summary.clear_live_lines();
        // 清空 summary 计数，避免后续 finalize 重复输出
        let _ = self.summary.finalize_reasoning_silent();
        let body = std::mem::take(&mut self.reasoning_full_buffer);
        let duration = self.reasoning_started.map(|started| started.elapsed());
        self.reasoning_started = None;
        let rendered = crate::render::transcript::reasoning_cell::render_thinking_body(
            &body, false, false, duration,
        );
        let mut stdout = io::stdout();
        if self.mode.is_some() {
            // 与前序输出空一行
            writeln!(stdout)?;
        }
        writeln!(stdout, "{rendered}")?;
        stdout.flush()?;
        self.mode = None;
        Ok(())
    }

    /// 固化推理摘要。
    ///
    /// 返回:
    /// - 固化是否成功
    pub(super) fn finalize_reasoning_summary(&mut self) -> Result<()> {
        if self.reasoning_mode == ReasoningDisplayMode::Full {
            return self.flush_full_reasoning_block();
        }
        if self.reasoning_mode == ReasoningDisplayMode::Summary && self.summary.has_reasoning() {
            self.stop_waiting()?;
            self.summary.finalize_reasoning()?;
            self.mode = None;
        }
        Ok(())
    }

    /// 固化工具调用摘要。
    ///
    /// 返回:
    /// - 固化是否成功
    pub(super) fn finalize_tools_summary(&mut self) -> Result<()> {
        if self.tool_call_mode == ToolCallDisplayMode::Summary && self.summary.has_tools() {
            self.stop_waiting()?;
            self.summary.finalize_tools()?;
        }
        Ok(())
    }

    /// 在末行显示/刷新当前工作状态动效。
    ///
    /// 参数:
    /// - `sub_phase`: 可选副状态（例如等待时的模型信息）
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn ensure_work_spinner(&mut self, sub_phase: Option<String>) -> Result<()> {
        if self.plain || !WaitSpinner::supported() {
            return Ok(());
        }
        let status = self.work_status.unwrap_or(WorkStatus::Working);
        let phase = status.localized_label().to_string();
        let started_at = *self.work_started.get_or_insert_with(Instant::now);
        if let Some(spinner) = self.wait_spinner.as_ref() {
            spinner.set_phase(phase);
            spinner.set_sub_phase(sub_phase);
            return Ok(());
        }
        self.hide_cursor()?;
        self.wait_spinner = Some(WaitSpinner::start_with_clock(
            phase,
            SpinnerStyle::Braille,
            sub_phase,
            started_at,
        ));
        Ok(())
    }

    /// 设置工作状态；有思考正文输出时不显示末行文案，其余场景在末行刷新。
    ///
    /// 参数:
    /// - `status`: 工作状态
    /// - `show_spinner`: 是否在末行显示动效
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn set_work_status(&mut self, status: WorkStatus, show_spinner: bool) -> Result<()> {
        self.work_status = Some(status);
        if show_spinner {
            self.ensure_work_spinner(None)
        } else {
            self.stop_waiting()
        }
    }

    /// 工具/内容输出后恢复末行工作动效。
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn resume_work_spinner(&mut self) -> Result<()> {
        if self.work_status.is_none() {
            return Ok(());
        }
        // 命令输出预览已内嵌 working 动效，不再启动 WaitSpinner
        if self.command_preview.is_active() {
            return Ok(());
        }
        // 思考正文流式输出期间不叠 working 文案；工具阶段显示工作中
        if self.mode == Some(ChatStreamKind::Reasoning)
            && self.reasoning_mode != ReasoningDisplayMode::Summary
        {
            return Ok(());
        }
        if self.mode == Some(ChatStreamKind::Content) {
            return Ok(());
        }
        self.ensure_work_spinner(None)
    }

    /// 停止等待动画。
    ///
    /// 返回:
    /// - 停止是否成功
    pub(super) fn stop_waiting(&mut self) -> Result<()> {
        if let Some(mut spinner) = self.wait_spinner.take() {
            spinner.stop()?;
        }
        Ok(())
    }

    /// 追加写入工具状态事件。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `status`: 工具状态文本
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn write_tool_event_line(&self, name: &str, status: &str) -> Result<()> {
        let label = self
            .tool_event_labels
            .get(name)
            .map(String::as_str)
            .unwrap_or_else(|| self.summary.display_tool_name(name));
        self.write_custom_tool_event_line(label, status)
    }

    /// 以指定标签写入工具状态事件。
    ///
    /// 参数:
    /// - `label`: 已格式化的工具显示名称
    /// - `status`: 工具状态文本
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn write_custom_tool_event_line(&self, label: &str, status: &str) -> Result<()> {
        let mut stdout = io::stdout();
        writeln!(stdout, "{}", tool_event_text(label, status))?;
        stdout.flush()?;
        Ok(())
    }
}

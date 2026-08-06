use super::*;

impl StreamRenderer {
    /// 【终端】【CLI 布局】渲染并对齐一段流式 Markdown 增量。
    ///
    /// 参数:
    /// - `text`: 新收到的 Markdown 文本
    ///
    /// 返回:
    /// - 正文位于视觉引导列右侧的 ANSI 文本
    pub(super) fn render_markdown_delta(&mut self, text: &str) -> String {
        let rendered = crate::render::render_width::with_render_width(
            crate::render::content_indent::cli_content_width(),
            || self.markdown.push(text),
        );
        crate::render::content_indent::align_cli_stream_block(
            &crate::render::content_indent::wrap_cli_stream_block(&rendered),
        )
    }

    /// 【终端】【CLI 布局】刷新并对齐 Markdown 渲染器剩余内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 正文位于视觉引导列右侧的 ANSI 文本
    pub(super) fn flush_markdown_output(&mut self) -> String {
        let rendered = crate::render::render_width::with_render_width(
            crate::render::content_indent::cli_content_width(),
            || self.markdown.flush(),
        );
        crate::render::content_indent::align_cli_stream_block(
            &crate::render::content_indent::wrap_cli_stream_block(&rendered),
        )
    }

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
            write!(stdout, "{}", self.flush_markdown_output())?;
            stdout.flush()?;
        }
        if self.mode.is_some() {
            println!();
            self.mode = None;
        }
        Ok(())
    }

    /// 【终端】【思考流式】按当前累计正文重绘 live 思考块。
    ///
    /// 先擦掉上一帧占用的视觉行，再写入新一帧，使正文随增量向下生长，
    /// 而不是把整段思考攒到结束才一次性输出。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 重绘是否成功
    pub(super) fn redraw_live_reasoning_block(&mut self) -> Result<()> {
        if self.reasoning_full_buffer.trim().is_empty() {
            return Ok(());
        }
        // 1. 逐帧重绘依赖光标上移擦除上一帧，非终端环境下这些序列不生效，
        //    每帧都会留在输出里堆叠成重复正文，因此只在真实终端上重绘
        if !WaitSpinner::supported() {
            return Ok(());
        }
        // 2. 生成本帧：标题带扫光动效，正文折叠为固定高度，避免长思考铺满屏幕
        self.reasoning_frame = self.reasoning_frame.wrapping_add(1);
        let elapsed = self
            .reasoning_started
            .map(|started| started.elapsed())
            .unwrap_or_default();
        let rendered = align_to_guide_column(&reasoning_cell::render_live(
            &self.reasoning_full_buffer,
            ReasoningDisplayMode::Full,
            self.reasoning_frame,
            elapsed,
            false,
        ));
        // 3. 擦除上一帧再写入本帧，行数按终端折行后的视觉行计算
        let mut stdout = io::stdout();
        if self.reasoning_live_rows > 0 {
            write!(stdout, "{}", clear_rendered_rows(self.reasoning_live_rows))?;
        }
        let block = format!("{rendered}\n");
        write!(stdout, "{block}")?;
        stdout.flush()?;
        self.reasoning_live_rows = rendered_visual_rows(&block);
        Ok(())
    }

    /// 将 Full 模式的 live 思考块定格为最终折叠块。
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn flush_full_reasoning_block(&mut self) -> Result<()> {
        if self.reasoning_full_buffer.trim().is_empty() {
            self.mode = None;
            self.reasoning_started = None;
            self.reasoning_live_rows = 0;
            self.reasoning_frame = 0;
            let _ = self.summary.clear_live_lines();
            return Ok(());
        }
        // 1. 擦除最后一帧 live 正文，改写为不含动效的定稿块
        let mut stdout = io::stdout();
        if self.reasoning_live_rows > 0 {
            write!(stdout, "{}", clear_rendered_rows(self.reasoning_live_rows))?;
            stdout.flush()?;
            self.reasoning_live_rows = 0;
        }
        let _ = self.summary.clear_live_lines();
        // 清空 summary 计数，避免后续 finalize 重复输出
        let _ = self.summary.finalize_reasoning_silent();
        let body = std::mem::take(&mut self.reasoning_full_buffer);
        let duration = self.reasoning_started.map(|started| started.elapsed());
        self.reasoning_started = None;
        self.reasoning_frame = 0;
        let rendered = align_to_guide_column(
            &crate::render::transcript::reasoning_cell::render_thinking_body(
                &body, false, false, duration,
            ),
        );
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
        self.wait_spinner = Some(WaitSpinner::start_with_clock(phase, sub_phase, started_at));
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

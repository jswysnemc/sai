use super::*;

impl StreamRenderer {
    /// 停止并清除前台命令输出预览动效。
    ///
    /// 返回:
    /// - 是否成功
    pub(super) fn stop_command_preview(&mut self) -> Result<()> {
        if self.command_preview.is_active() {
            self.command_preview.clear()?;
        }
        Ok(())
    }

    /// 写入模型流式文本片段。
    ///
    /// 参数:
    /// - `chunk`: 模型流式片段
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_chunk(&mut self, chunk: ChatStreamChunk) -> Result<()> {
        self.hide_cursor()?;
        // 正文/思考到来时结束命令预览动效，防止相对清屏擦掉流式输出
        self.stop_command_preview()?;
        let text = normalize_stream_text(&chunk.text);
        if self.plain && chunk.kind == ChatStreamKind::Reasoning {
            return Ok(());
        }
        if self.reasoning_mode == ReasoningDisplayMode::Hidden
            && chunk.kind == ChatStreamKind::Reasoning
        {
            return Ok(());
        }
        if self.reasoning_mode == ReasoningDisplayMode::Summary
            && chunk.kind == ChatStreamKind::Reasoning
        {
            // Summary 模式：一开始就显示思考块，并随 chunk 更新行数/字符数
            self.finish_live_tool_status()?;
            self.finalize_tools_summary()?;
            if self.mode != Some(ChatStreamKind::Reasoning) {
                self.reasoning_started = Some(Instant::now());
            }
            let elapsed = self
                .reasoning_started
                .map(|started| started.elapsed())
                .unwrap_or_default();
            self.summary
                .add_reasoning_text_with_elapsed(&text, elapsed)?;
            self.mode = Some(ChatStreamKind::Reasoning);
            // 有思考内容时不显示 working 文案
            self.set_work_status(WorkStatus::Thinking, false)?;
            return Ok(());
        }
        // 流式正文/思考期间不在末行叠状态；工具间隙再恢复
        self.set_work_status(
            match chunk.kind {
                ChatStreamKind::Reasoning => WorkStatus::Thinking,
                ChatStreamKind::Content => WorkStatus::Working,
            },
            false,
        )?;
        self.finish_live_tool_status()?;
        // Full 模式思考：缓存正文并刷新与 Summary 一致的 live 摘要行，结束时折叠输出
        if self.reasoning_mode == ReasoningDisplayMode::Full
            && chunk.kind == ChatStreamKind::Reasoning
        {
            if self.mode != Some(ChatStreamKind::Reasoning) {
                self.mode = Some(ChatStreamKind::Reasoning);
                self.reasoning_full_buffer.clear();
                self.reasoning_started = Some(Instant::now());
            }
            self.reasoning_full_buffer.push_str(&text);
            // live 行：动效 + tokens + thinking(耗时)
            self.summary.add_reasoning_text_with_elapsed(
                &text,
                self.reasoning_started
                    .map(|s| s.elapsed())
                    .unwrap_or_default(),
            )?;
            return Ok(());
        }
        if self.mode != Some(chunk.kind) {
            if chunk.kind == ChatStreamKind::Content {
                self.flush_full_reasoning_block()?;
                self.finalize_reasoning_summary()?;
                self.finalize_tools_summary()?;
            }
            self.switch_mode(chunk.kind)?;
        }
        let mut stdout = io::stdout();
        if self.plain {
            write!(stdout, "{text}")?;
        } else {
            write!(stdout, "{}", self.markdown.push(&text))?;
        }
        stdout.flush()?;
        Ok(())
    }

    /// 写入工具调用。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_tool_call(&mut self, name: &str, arguments: &str) -> Result<()> {
        if self.plain {
            return Ok(());
        }
        self.set_work_status(WorkStatus::Working, false)?;
        let background_command_start =
            name == "background_command" && is_background_command_start(arguments);
        let event_label = tool_event_label(name, Some(arguments));
        self.tool_event_labels
            .insert(name.to_string(), event_label.clone());
        if name == "run_command" {
            // 命令输出预览接管底部动效，停掉 WaitSpinner 避免锚点冲突
            self.stop_waiting()?;
            self.command_preview.begin();
        } else {
            // 其它工具写入前必须停掉上一条命令预览，否则会相对上移擦屏
            self.stop_command_preview()?;
        }
        if self.tool_call_mode == ToolCallDisplayMode::Summary
            && !tool_call_has_visible_block(name)
            && !background_command_start
        {
            self.write_live_tool_status(&event_label, tool_start_status(name), false)?;
            self.resume_work_spinner()?;
            return Ok(());
        }
        self.clear_live_tool_status()?;
        self.end_active_stream_line()?;
        self.finalize_reasoning_summary()?;
        if name == "run_command" || background_command_start {
            self.summary.clear_live_lines()?;
            if write_command_tool_call_block(
                name,
                arguments,
                background_command_start,
                &mut self.command_block_tools,
            )? {
                self.resume_work_spinner()?;
                return Ok(());
            }
        }
        if crate::render::stream_text::is_file_edit_tool(name) {
            self.summary.clear_live_lines()?;
            if self.pending_streamed_edit_blocks > 0 {
                self.pending_streamed_edit_blocks -= 1;
                self.resume_work_spinner()?;
                return Ok(());
            }
            if write_edit_tool_call_block(name, arguments)? {
                self.resume_work_spinner()?;
                return Ok(());
            }
        }
        if self.tool_call_mode == ToolCallDisplayMode::Full {
            self.summary.clear_live_lines()?;
            let mut stdout = io::stdout();
            writeln!(
                stdout,
                "{}",
                tool_view::render_call(name, arguments, self.tool_call_mode)
            )?;
            stdout.flush()?;
        }
        self.resume_work_spinner()?;
        Ok(())
    }

    /// 写入工具调用参数接收进度。
    ///
    /// 参数进度只更新单行 live 状态；完整命令块等 ToolCall 定稿后一次性输出，
    /// 避免多行预览反复清除重绘在宽字符或滚动场景下残留重复内容。
    ///
    /// 参数:
    /// - `progress`: 工具调用参数流式进度
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_tool_call_progress(&mut self, progress: &ToolCallStreamProgress) -> Result<()> {
        if self.plain {
            return Ok(());
        }
        self.set_work_status(WorkStatus::Working, false)?;
        if self.tool_call_mode == ToolCallDisplayMode::Hidden {
            return Ok(());
        }
        let name = progress.name.as_deref().unwrap_or("tool");
        if name != "run_command" {
            self.stop_command_preview()?;
        }
        let event_label = tool_event_label(name, Some(&progress.arguments_preview));
        self.tool_event_labels
            .insert(name.to_string(), event_label.clone());
        if crate::render::stream_text::is_file_edit_tool(name)
            && !self.streaming_edit_progress.contains(&progress.index)
        {
            self.clear_live_tool_status()?;
            self.end_active_stream_line()?;
            self.finalize_reasoning_summary()?;
            self.summary.clear_live_lines()?;
            if write_edit_tool_call_diff_block(name, &progress.arguments_preview)? {
                self.streaming_edit_progress.insert(progress.index);
                self.pending_streamed_edit_blocks += 1;
                self.resume_work_spinner()?;
                return Ok(());
            }
        }
        self.write_live_tool_status(&event_label, "arg", false)?;
        self.resume_work_spinner()?;
        Ok(())
    }
}

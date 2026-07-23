use super::*;

impl StreamRenderer {
    /// 写入工具结果。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `ok`: 工具是否成功
    /// - `output`: 工具输出
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_tool_result(&mut self, name: &str, ok: bool, output: &str) -> Result<()> {
        if self.plain {
            return Ok(());
        }
        self.set_work_status(WorkStatus::Working, false)?;
        if name != "run_command" {
            self.stop_command_preview()?;
        }
        // 权限拒绝的失败结果已由「已拒绝」决定行呈现，跳过重复的输出块
        let suppressed = self.suppressed_denied_results.remove(name);
        if suppressed && !ok {
            if self.tool_call_mode == ToolCallDisplayMode::Summary {
                self.summary.note_tool_result(name, ok);
            }
            self.finish_live_tool_status()?;
            self.resume_work_spinner()?;
            return Ok(());
        }
        let status = if ok { "ok" } else { "err" };
        let event_label = self
            .tool_event_labels
            .get(name)
            .cloned()
            .unwrap_or_else(|| tool_event_label(name, None));
        let background_result_label = if name == "background_command" && ok {
            background_command_result_label(output)
        } else {
            None
        };
        let command_block_result = self.command_block_tools.remove(name);
        if self.tool_call_mode == ToolCallDisplayMode::Summary {
            if tool_call_has_visible_block(name) || command_block_result {
                if name == "run_command" {
                    let mut stdout = io::stdout();
                    self.stop_waiting()?;
                    self.command_preview.clear()?;
                    // 命令结束后恢复末行 WaitSpinner
                    if ok {
                        write_command_result_preview(&mut stdout, output)?;
                    } else {
                        write_command_error_preview(&mut stdout, output)?;
                    }
                    stdout.flush()?;
                    self.resume_work_spinner()?;
                    return Ok(());
                }
                if command_block_result {
                    if let Some(label) = background_result_label {
                        self.write_custom_tool_event_line(&label, status)?;
                        self.resume_work_spinner()?;
                        return Ok(());
                    }
                    if !ok {
                        self.write_tool_event_line(name, status)?;
                    }
                    self.resume_work_spinner()?;
                    return Ok(());
                }
                if !ok {
                    self.write_tool_event_line(name, status)?;
                }
                self.resume_work_spinner()?;
                return Ok(());
            }
            self.write_live_tool_status(
                background_result_label.as_deref().unwrap_or(&event_label),
                status,
                true,
            )?;
            self.resume_work_spinner()?;
            return Ok(());
        }
        self.finish_live_tool_status()?;
        if name == "run_command" {
            let mut stdout = io::stdout();
            self.stop_waiting()?;
            self.command_preview.clear()?;
            if ok {
                write_command_result_preview(&mut stdout, output)?;
            } else {
                write_command_error_preview(&mut stdout, output)?;
            }
            stdout.flush()?;
            self.resume_work_spinner()?;
            return Ok(());
        }
        if command_block_result && ok {
            if let Some(label) = background_result_label {
                let mut stdout = io::stdout();
                writeln!(stdout, "{}", tool_event_text(&label, status))?;
                if self.tool_call_mode == ToolCallDisplayMode::Full {
                    write_tool_payload(&mut stdout, t("output", "输出"), output)?;
                }
                stdout.flush()?;
            }
            self.resume_work_spinner()?;
            return Ok(());
        }
        if tool_call_has_visible_block(name) && ok {
            self.resume_work_spinner()?;
            return Ok(());
        }
        if self.tool_call_mode == ToolCallDisplayMode::Full {
            let mut stdout = io::stdout();
            if let Some(label) = background_result_label.as_deref() {
                writeln!(stdout, "{}", tool_event_text(label, status))?;
                write_tool_payload(&mut stdout, t("output", "输出"), output)?;
            } else {
                writeln!(
                    stdout,
                    "{}",
                    tool_view::render_result(name, ok, output, self.tool_call_mode)
                )?;
            }
            stdout.flush()?;
        } else if self.tool_call_mode == ToolCallDisplayMode::Summary {
            self.summary.note_tool_result(name, ok);
            if !tool_call_has_visible_block(name) {
                if let Some(label) = background_result_label {
                    self.write_custom_tool_event_line(&label, status)?;
                } else {
                    self.write_tool_event_line(name, status)?;
                }
            }
        }
        self.resume_work_spinner()?;
        Ok(())
    }

    /// 写入工具进度。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `message`: 进度信息
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_tool_progress(&mut self, name: &str, message: &str) -> Result<()> {
        if self.plain {
            return Ok(());
        }
        self.set_work_status(WorkStatus::Working, false)?;
        if message == "__external_output__" {
            self.prepare_for_external_output()?;
            return Ok(());
        }
        if let Some(text) = message.strip_prefix("__subagent_reasoning__") {
            return self.write_subagent_reasoning(name, text);
        }
        if let Some(payload) = message.strip_prefix("__subtool_call__") {
            return self.write_subtool_call(name, payload);
        }
        if let Some(payload) = message.strip_prefix("__subtool_result__") {
            return self.write_subtool_result(name, payload);
        }
        if name == "run_command" {
            if let Some(chunk) = crate::tools::command::decode_command_output(message) {
                // 1. 停止 WaitSpinner，改由命令预览内嵌 working 动效
                self.stop_waiting()?;
                self.end_active_stream_line()?;
                self.finalize_reasoning_summary()?;
                // 2. 命令预览独占底部行，避免 clear_rendered_rows 与 spinner 锚点冲突
                self.command_preview.append(&chunk)?;
                return Ok(());
            }
        }
        self.stop_waiting()?;
        self.end_active_stream_line()?;
        self.finalize_reasoning_summary()?;
        if self.tool_call_mode == ToolCallDisplayMode::Full {
            let mut stdout = io::stdout();
            writeln!(
                stdout,
                "{TOOL_BULLET} progress {}: {message}",
                self.summary.display_tool_name(name)
            )?;
            stdout.flush()?;
        } else if self.tool_call_mode == ToolCallDisplayMode::Summary {
            self.summary.note_tool_progress(name, message);
        }
        self.resume_work_spinner()?;
        Ok(())
    }

    /// 写入上下文压缩开始事件。
    ///
    /// 参数:
    /// - `turn_count`: 本次计划压缩的轮次数量
    /// - `model`: 压缩模型标签
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_compaction_started(&mut self, turn_count: usize, model: &str) -> Result<()> {
        if self.plain {
            return Ok(());
        }
        self.stop_waiting()?;
        self.summary.clear_live_lines()?;
        self.end_active_stream_line()?;
        self.finalize_reasoning_summary()?;
        self.finalize_tools_summary()?;
        let mut stdout = io::stdout();
        writeln!(
            stdout,
            "{}",
            tool_event_text(
                &format!(
                    "{}×{turn_count} · {model}",
                    t("compact context", "压缩上下文")
                ),
                "run"
            )
        )?;
        stdout.flush()?;
        self.work_status = Some(WorkStatus::Compacting);
        self.ensure_work_spinner(wait_spinner_detail_line(&self.options))
    }

    /// 写入压缩模型的流式 Markdown 增量。
    ///
    /// 参数:
    /// - `text`: 压缩摘要正文增量
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_compaction_delta(&mut self, text: String) -> Result<()> {
        self.write_chunk(ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text,
        })
    }

    /// 写入上下文压缩结束事件。
    ///
    /// 参数:
    /// - `applied`: 是否成功应用压缩结果
    /// - `error`: 可选失败详情
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn write_compaction_finished(
        &mut self,
        applied: bool,
        error: Option<&crate::agent::CompactionError>,
    ) -> Result<()> {
        if self.plain {
            return Ok(());
        }
        self.stop_waiting()?;
        self.summary.clear_live_lines()?;
        self.end_active_stream_line()?;
        self.finalize_reasoning_summary()?;
        let status = if applied { "ok" } else { "skip" };
        let mut stdout = io::stdout();
        writeln!(
            stdout,
            "{}",
            tool_event_text(t("compact context", "压缩上下文"), status)
        )?;
        if let Some(error) = error {
            writeln!(stdout, "{}", error.message)?;
            if !error.detail.trim().is_empty() && error.detail != error.message {
                writeln!(stdout, "{}", error.detail)?;
            }
        }
        stdout.flush()?;
        Ok(())
    }
}

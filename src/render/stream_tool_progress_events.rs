use super::*;

impl StreamRenderer {
    /// 写入子代理推理进度。
    ///
    /// 参数:
    /// - `name`: 父工具名称
    /// - `text`: 子代理推理文本
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn write_subagent_reasoning(&mut self, name: &str, text: &str) -> Result<()> {
        let text = normalize_stream_text(text);
        if self.tool_call_mode == ToolCallDisplayMode::Hidden || text.trim().is_empty() {
            return Ok(());
        }
        if self.tool_call_mode == ToolCallDisplayMode::Full {
            self.stop_waiting()?;
            self.end_active_stream_line()?;
            self.finalize_reasoning_summary()?;
            let mut stdout = io::stdout();
            execute!(stdout, SetForegroundColor(Color::Green))?;
            write!(stdout, "{text}")?;
            execute!(stdout, ResetColor)?;
            stdout.flush()?;
            return Ok(());
        }
        let line_count = text.matches('\n').count().max(1);
        self.summary.note_tool_progress(
            name,
            &format!("{} · {} {}", "subagent reasoning", line_count, "lines"),
        );
        Ok(())
    }

    /// 写入子工具调用进度。
    ///
    /// 参数:
    /// - `parent_name`: 父工具名称
    /// - `payload`: 子工具调用 JSON
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn write_subtool_call(&mut self, parent_name: &str, payload: &str) -> Result<()> {
        let value = serde_json::from_str::<Value>(payload).unwrap_or(Value::Null);
        let tool_name = value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let args = value.get("args").and_then(Value::as_str).unwrap_or("");
        if self.tool_call_mode == ToolCallDisplayMode::Full {
            self.stop_waiting()?;
            self.end_active_stream_line()?;
            self.finalize_reasoning_summary()?;
            let mut stdout = io::stdout();
            if tool_name == "run_command" {
                // 命令内容已在代码块内展示，标题固定用动作短名避免重复
                write_command_block_with_action(&mut stdout, args, "Run")?;
            } else if tool_name == "edit_file" {
                if !write_edit_file_diff_block(&mut stdout, args)? {
                    write_tool_payload(&mut stdout, "args", args)?;
                }
            } else {
                writeln!(
                    stdout,
                    "{}",
                    tool_event_text(&tool_event_label(tool_name, Some(args)), "run")
                )?;
                write_tool_payload(&mut stdout, "args", args)?;
            }
            stdout.flush()?;
        } else if self.tool_call_mode == ToolCallDisplayMode::Summary {
            self.summary.note_tool_progress(
                parent_name,
                &format!(
                    "{}: {}",
                    "subtool running",
                    self.summary.display_tool_name(tool_name)
                ),
            );
        }
        Ok(())
    }

    /// 写入子工具结果进度。
    ///
    /// 参数:
    /// - `parent_name`: 父工具名称
    /// - `payload`: 子工具结果 JSON
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn write_subtool_result(&mut self, parent_name: &str, payload: &str) -> Result<()> {
        let value = serde_json::from_str::<Value>(payload).unwrap_or(Value::Null);
        let tool_name = value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(true);
        let output = value.get("output").and_then(Value::as_str).unwrap_or("");
        let status = if ok { "ok" } else { "err" };
        if self.tool_call_mode == ToolCallDisplayMode::Full {
            self.stop_waiting()?;
            self.end_active_stream_line()?;
            self.finalize_reasoning_summary()?;
            let mut stdout = io::stdout();
            writeln!(
                stdout,
                "{}",
                tool_event_text(&tool_event_label(tool_name, None), status)
            )?;
            write_tool_payload(&mut stdout, "output", output)?;
            stdout.flush()?;
        } else if self.tool_call_mode == ToolCallDisplayMode::Summary {
            self.summary.note_tool_progress(
                parent_name,
                &format!(
                    "{}: {} {status}",
                    "subtool finished",
                    self.summary.display_tool_name(tool_name)
                ),
            );
        }
        Ok(())
    }
}

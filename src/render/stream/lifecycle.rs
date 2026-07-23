use super::*;

impl StreamRenderer {
    /// 为外部程序直接输出终端内容做准备。
    ///
    /// 返回:
    /// - 准备是否成功
    pub fn prepare_for_external_output(&mut self) -> Result<()> {
        self.stop_waiting()?;
        self.stop_command_preview()?;
        // 权限交互期间不要恢复末行 working 动效
        self.work_status = None;
        self.work_started = None;
        // 先固化思考摘要，再清 live / 切出流式行
        self.finalize_reasoning_summary()?;
        self.summary.clear_live_lines()?;
        self.finish_live_tool_status()?;
        self.end_active_stream_line()?;
        self.finalize_tools_summary()?;
        self.show_cursor()?;
        let mut stdout = io::stdout();
        stdout.flush()?;
        Ok(())
    }

    /// 立即刷新当前正文缓冲。
    ///
    /// 返回:
    /// - 刷新是否成功
    pub fn flush_content(&mut self) -> Result<()> {
        if self.mode != Some(ChatStreamKind::Content) {
            return Ok(());
        }
        if self.plain {
            println!();
        } else {
            let mut stdout = io::stdout();
            write!(stdout, "{}", self.markdown.flush())?;
            stdout.flush()?;
        }
        self.mode = None;
        Ok(())
    }

    /// 完成当前流式渲染。
    ///
    /// 返回:
    /// - 收尾是否成功
    pub fn finish(&mut self) -> Result<()> {
        self.stop_waiting()?;
        self.stop_command_preview()?;
        self.finish_live_tool_status()?;
        // Summary 思考 live 行先定格，避免后面的 println 拆成两行
        if self.reasoning_mode == ReasoningDisplayMode::Summary {
            self.finalize_reasoning_summary()?;
        }
        self.summary.clear_live_lines()?;
        if self.mode == Some(ChatStreamKind::Content) && !self.plain {
            let mut stdout = io::stdout();
            write!(stdout, "{}", self.markdown.flush())?;
            stdout.flush()?;
        }
        if self.mode == Some(ChatStreamKind::Reasoning)
            && self.reasoning_mode != ReasoningDisplayMode::Summary
        {
            execute!(io::stdout(), ResetColor)?;
            println!();
        } else if self.mode == Some(ChatStreamKind::Content) {
            println!();
        }
        if self.reasoning_mode != ReasoningDisplayMode::Summary {
            self.finalize_reasoning_summary()?;
        }
        self.finalize_tools_summary()?;
        self.mode = None;
        self.work_status = None;
        self.work_started = None;
        self.stop_waiting()?;
        self.show_cursor()?;
        Ok(())
    }
}

impl Drop for StreamRenderer {
    /// 释放渲染器时停止等待状态并恢复终端显示属性。
    ///
    /// 返回:
    /// - 无；清理错误在析构阶段忽略
    fn drop(&mut self) {
        let _ = self.stop_waiting();
        let _ = self.summary.clear_live_lines();
        let _ = self.show_cursor();
        let _ = execute!(io::stdout(), ResetColor);
    }
}

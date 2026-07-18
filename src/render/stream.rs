use crate::i18n::text as t;
use crate::llm::{ChatStreamChunk, ChatStreamKind, ToolCallStreamProgress};
use crate::render::background_command_event::{
    background_command_result_label, is_background_command_start,
};
use crate::render::command_output::{
    write_command_block_with_action, write_command_error_block, write_command_result_blocks,
    write_tool_payload,
};
use crate::render::edit_diff::write_edit_file_diff_block;
use crate::render::live_tool_status::LiveToolStatus;
use crate::render::markdown::MarkdownStreamRenderer;
use crate::render::stream_config::{
    ReasoningDisplayMode, StreamRenderOptions, ToolCallDisplayMode,
};
use crate::render::stream_summary::StreamSummary;
pub(crate) use crate::render::stream_text::{
    normalize_stream_text, tool_call_has_visible_block, wait_spinner_detail_line,
};
use crate::render::stream_tool_status::tool_start_status;
use crate::render::style::TOOL_BULLET;
use crate::render::tool_call_blocks::{
    write_command_tool_call_block, write_edit_tool_call_block, write_edit_tool_call_diff_block,
};
use crate::render::tool_event_line::{tool_event_label, tool_event_text};
use crate::render::tool_view;
use crate::render::wait_spinner::{SpinnerStyle, WaitSpinner};
use crate::render::work_status::WorkStatus;
use anyhow::Result;
use crossterm::execute;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};

include!("stream_renderer_output.rs");
#[path = "stream_tool_progress_events.rs"]
mod stream_tool_progress_events;

#[cfg(test)]
use crate::render::stream_summary::{
    style_summary_text, tool_status_text, SummaryStyle, ToolStats,
};

pub struct StreamRenderer {
    reasoning_mode: ReasoningDisplayMode,
    tool_call_mode: ToolCallDisplayMode,
    plain: bool,
    options: StreamRenderOptions,
    mode: Option<ChatStreamKind>,
    pub(crate) cursor_hidden: bool,
    markdown: MarkdownStreamRenderer,
    summary: StreamSummary,
    wait_spinner: Option<WaitSpinner>,
    live_tool_status: LiveToolStatus,
    tool_event_labels: HashMap<String, String>,
    command_block_tools: HashSet<String>,
    streaming_edit_progress: HashSet<usize>,
    pending_streamed_edit_blocks: usize,
    suppressed_denied_results: HashSet<String>,
    work_status: Option<WorkStatus>,
}

impl StreamRenderer {
    /// 创建流式响应渲染器。
    ///
    /// 参数:
    /// - `reasoning_mode`: 推理内容展示模式
    /// - `tool_call_mode`: 工具调用展示模式
    /// - `plain`: 是否使用纯文本输出
    /// - `options`: 流式渲染附加选项
    ///
    /// 返回:
    /// - 新的流式渲染器
    pub fn new(
        reasoning_mode: ReasoningDisplayMode,
        tool_call_mode: ToolCallDisplayMode,
        plain: bool,
        options: StreamRenderOptions,
    ) -> Self {
        let readable_tool_names = options.readable_tool_names;
        Self {
            reasoning_mode,
            tool_call_mode,
            plain,
            options,
            mode: None,
            cursor_hidden: false,
            markdown: MarkdownStreamRenderer::new(),
            summary: StreamSummary::new(readable_tool_names),
            wait_spinner: None,
            live_tool_status: LiveToolStatus::new(),
            tool_event_labels: HashMap::new(),
            command_block_tools: HashSet::new(),
            streaming_edit_progress: HashSet::new(),
            pending_streamed_edit_blocks: 0,
            suppressed_denied_results: HashSet::new(),
            work_status: None,
        }
    }

    /// 标记指定工具的下一条失败结果由权限拒绝产生，无需重复输出。
    ///
    /// 参数:
    /// - `tool`: 被拒绝的工具名称
    ///
    /// 返回:
    /// - 无
    pub fn suppress_denied_result(&mut self, tool: &str) {
        self.suppressed_denied_results.insert(tool.to_string());
    }

    /// 启动等待响应动画。
    ///
    /// 返回:
    /// - 启动是否成功
    pub fn start_waiting(&mut self) -> Result<()> {
        self.work_status = Some(WorkStatus::WaitingResponse);
        self.ensure_work_spinner(wait_spinner_detail_line(&self.options))
    }

    /// 显示等待后台命令或子 Agent 完成的状态。
    ///
    /// 返回:
    /// - 启动是否成功
    pub fn start_waiting_external(&mut self) -> Result<()> {
        self.work_status = Some(WorkStatus::WaitingExternal);
        self.ensure_work_spinner(wait_spinner_detail_line(&self.options))
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
            self.summary.add_reasoning_text(&text)?;
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
        if self.mode != Some(chunk.kind) {
            if chunk.kind == ChatStreamKind::Content {
                self.finalize_reasoning_summary()?;
                self.finalize_tools_summary()?;
            }
            self.switch_mode(chunk.kind)?;
        }
        let mut stdout = io::stdout();
        if self.plain || chunk.kind == ChatStreamKind::Reasoning {
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
        if name == "edit_file" {
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
        let event_label = tool_event_label(name, Some(&progress.arguments_preview));
        self.tool_event_labels
            .insert(name.to_string(), event_label.clone());
        if name == "edit_file" && !self.streaming_edit_progress.contains(&progress.index) {
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

    /// 写入单行工具状态。
    ///
    /// 参数:
    /// - `name`: 工具展示标签
    /// - `status`: 工具状态，取值为 arg、run、ok 或 err
    /// - `final_line`: 是否结束当前状态行
    ///
    /// 返回:
    /// - 写入是否成功
    fn write_live_tool_status(&mut self, name: &str, status: &str, final_line: bool) -> Result<()> {
        if !self.live_tool_status.is_active() {
            self.end_active_stream_line()?;
            self.finalize_reasoning_summary()?;
        }
        self.live_tool_status
            .write(self.summary.display_tool_name(name), status, final_line)
    }

    /// 结束当前单行工具状态。
    ///
    /// 返回:
    /// - 写入是否成功
    fn finish_live_tool_status(&mut self) -> Result<()> {
        self.live_tool_status.finish()
    }

    /// 清除当前单行工具状态。
    ///
    /// 返回:
    /// - 写入是否成功
    fn clear_live_tool_status(&mut self) -> Result<()> {
        self.live_tool_status.clear()
    }

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
                if name == "run_command" && !ok {
                    let mut stdout = io::stdout();
                    write_command_error_block(&mut stdout, output)?;
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
        if name == "run_command" && self.tool_call_mode == ToolCallDisplayMode::Full {
            let mut stdout = io::stdout();
            write_command_result_blocks(&mut stdout, output)?;
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

    /// 为外部程序直接输出终端内容做准备。
    ///
    /// 返回:
    /// - 准备是否成功
    pub fn prepare_for_external_output(&mut self) -> Result<()> {
        self.stop_waiting()?;
        // 权限交互期间不要恢复末行 working 动效
        self.work_status = None;
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
        self.stop_waiting()?;
        self.show_cursor()?;
        Ok(())
    }
}

impl Drop for StreamRenderer {
    fn drop(&mut self) {
        let _ = self.stop_waiting();
        let _ = self.summary.clear_live_lines();
        let _ = self.show_cursor();
        let _ = execute!(io::stdout(), ResetColor);
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;

use crate::llm::{ChatStreamChunk, ChatStreamKind, ToolCallStreamProgress};
use crate::render::background_command_event::{
    background_command_result_label, is_background_command_start,
};
use crate::render::cli_command_preview::CliCommandPreview;
use crate::render::command_output::{
    write_command_block_with_action, write_command_error_preview, write_command_result_preview,
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
use crate::render::terminal_text as t;
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
use std::time::Instant;

mod input_events;
mod lifecycle;
mod live_status;
mod output;
mod tool_events;
mod tool_progress;

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
    /// 本轮工作动效计时起点（首次进入工作态时建立，整轮不重置）
    work_started: Option<Instant>,
    /// Full 模式下缓存思考正文，结束后按折叠块输出
    reasoning_full_buffer: String,
    /// 当前思考段开始时间（用于 live 耗时）
    reasoning_started: Option<Instant>,
    command_preview: CliCommandPreview,
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
            work_started: None,
            reasoning_full_buffer: String::new(),
            reasoning_started: None,
            command_preview: CliCommandPreview::new(),
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
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;

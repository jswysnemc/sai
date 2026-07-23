mod operations;

use super::cell::HistoryCell;
use super::render_cache::RenderCache;
use crate::llm::ChatStreamKind;
use crate::render::work_status::WorkStatus;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use std::time::Instant;

/// REPL transcript 的渲染选项快照。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TranscriptRenderOptions {
    pub(crate) reasoning_mode: ReasoningDisplayMode,
    pub(crate) tool_call_mode: ToolCallDisplayMode,
}

/// 仍在生成中的文本 source。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveTail {
    pub(super) kind: ChatStreamKind,
    pub(super) source: String,
}

/// 正在接收参数的工具调用预览。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveToolCall {
    pub(super) name: String,
    pub(super) arguments_preview: String,
}

/// 保存 REPL 会话的定稿 cell 与可变流式尾部。
pub(crate) struct TranscriptStore {
    pub(super) cells: Vec<HistoryCell>,
    pub(super) live_tail: Option<LiveTail>,
    pub(super) live_tool_call: Option<LiveToolCall>,
    pub(super) live_animation_frame: usize,
    pub(super) active_tool_index: Option<usize>,
    pub(super) work_status: Option<WorkStatus>,
    pub(super) work_status_started: Option<Instant>,
    pub(super) row_cap: usize,
    pub(super) cache: RenderCache,
    pub(super) dirty_from_cell: Option<usize>,
}

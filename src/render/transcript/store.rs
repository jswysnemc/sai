mod live_reasoning;
mod operations;
mod subagent_panel;
mod todo_snapshot;

pub(crate) use subagent_panel::SubagentOverviewEntry;

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
    pub(super) expanded: bool,
}

/// 正在接收参数的工具调用预览。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveToolCall {
    pub(super) name: String,
    pub(super) arguments_preview: String,
}

/// 最近一次 todo 工具快照中的单个条目（供沉底面板展示）。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TodoSnapshotItem {
    pub(crate) status: String,
    pub(crate) text: String,
}

/// transcript 当前展示的会话视图。
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub(crate) enum TranscriptView {
    /// 主 agent 会话
    #[default]
    Main,
    /// 指定子智能体的会话时间线
    Subagent { id: String, label: String },
}

/// 保存 REPL 会话的定稿 cell 与可变流式尾部。
pub(crate) struct TranscriptStore {
    pub(super) cells: Vec<HistoryCell>,
    pub(super) live_tail: Option<LiveTail>,
    pub(super) live_tool_call: Option<LiveToolCall>,
    /// 当前流式段的动效起点。
    ///
    /// 帧号由起点到当前的时长换算，而不是每次刷新加一：后者让动效速度
    /// 取决于主循环的唤醒间隔，主循环一变动效就跟着快慢不定。
    pub(super) live_animation_started: Option<Instant>,
    /// 最近一次活动工具 cell；并发时真正的匹配走 FIFO 扫描，不依赖这一项。
    pub(super) active_tool_index: Option<usize>,
    pub(super) work_status: Option<WorkStatus>,
    pub(super) work_status_started: Option<Instant>,
    pub(super) row_cap: usize,
    pub(super) cache: RenderCache,
    pub(super) dirty_from_cell: Option<usize>,
    /// 最近一次 todo 工具结果携带的全量清单快照
    pub(super) latest_todo: Vec<TodoSnapshotItem>,
    /// 当前展示的会话视图（主 agent 或某个子智能体）
    pub(super) view: TranscriptView,
}

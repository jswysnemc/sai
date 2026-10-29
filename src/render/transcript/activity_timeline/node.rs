use super::super::cell::HistoryCell;
use super::super::line::AnsiLine;
use super::super::store::{TranscriptRenderOptions, TranscriptStore};
use super::super::tool_cell::ToolCell;
use super::{appearance, summary};
use crate::render::status_style::ToolHealth;
use crate::render::terminal_text as t;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

#[derive(Clone)]
pub(super) struct RenderedNode {
    pub(super) lines: Vec<AnsiLine>,
    pub(super) open: bool,
}
pub(super) enum LiveNode {
    Thought,
    Tool,
}

/// 【终端】【节点可见性】读取工具可见性偏好。
/// 参数: `options` 为偏好；返回: 是否允许显示工具。
pub(super) fn tools_visible(options: &TranscriptRenderOptions) -> bool {
    options.tool_call_mode != ToolCallDisplayMode::Hidden
}

/// 【终端】【节点可见性】读取思考可见性偏好。
/// 参数: `options` 为偏好；返回: 是否允许显示思考。
pub(super) fn thoughts_visible(options: &TranscriptRenderOptions) -> bool {
    options.reasoning_mode != ReasoningDisplayMode::Hidden
}

/// 【终端】【节点可见性】按步骤类型执行隐藏偏好，空思考不占用节点。
/// 参数: `cell` 为步骤，`options` 为偏好；返回: 是否可见。
pub(super) fn visible(cell: &HistoryCell, options: &TranscriptRenderOptions) -> bool {
    match cell {
        HistoryCell::Reasoning(cell) => thoughts_visible(options) && !cell.source.trim().is_empty(),
        _ => tools_visible(options),
    }
}

/// 【终端】【审批节点】识别必须始终显示审批控件的步骤。
/// 参数: `cell` 为步骤；返回: 是否尚未完成审批。
pub(super) fn awaiting_permission(cell: &HistoryCell) -> bool {
    match cell {
        HistoryCell::Tool(ToolCell::Invocation(view)) => view
            .permission
            .as_ref()
            .is_some_and(|permission| permission.decision.is_none()),
        HistoryCell::Diff(cell) => cell.has_pending_permission(),
        _ => false,
    }
}

/// 【终端】【节点详情】窗口内步骤保留详情，窗口外历史步骤按手动展开状态恢复全文。
/// 参数: `cell` 为步骤，`width` 为净宽，`active` 为是否当前步骤，`open` 为展开状态；返回: 节点。
pub(super) fn render_cell(
    cell: &HistoryCell,
    width: usize,
    active: bool,
    open: bool,
) -> RenderedNode {
    let animate = |mut node: RenderedNode| {
        let icon = match cell {
            HistoryCell::Tool(ToolCell::Invocation(view)) => {
                Some(crate::render::guide_symbols::tool(&view.name))
            }
            HistoryCell::Diff(_) => Some(crate::render::guide_symbols::EDIT),
            HistoryCell::Shell(_) => Some(crate::render::guide_symbols::COMMAND),
            HistoryCell::Tool(ToolCell::Subagent(_)) => Some(crate::render::guide_symbols::AGENTS),
            HistoryCell::Tool(
                ToolCell::CompactionStarted { .. } | ToolCell::CompactionFinished { .. },
            ) => Some(crate::render::guide_symbols::tool("compress_context")),
            _ => None,
        };
        if let Some(icon) = icon {
            let marker = if active
                && appearance::health(cell) == ToolHealth::Pending
                && !awaiting_permission(cell)
            {
                crate::render::guide_symbols::pulse(
                    icon,
                    crate::render::activity_animation::activity_frame_at(
                        crate::render::activity_animation::activity_started_at().elapsed(),
                    ),
                )
            } else {
                icon.to_string()
            };
            super::pulse::apply_marker(&mut node.lines, &marker);
        }
        node
    };
    if let Some(node) = super::standard_node::render_cell(cell, width, open) {
        return animate(node);
    }
    let title = summary::for_cell(cell, width, awaiting_permission(cell));
    let body_width = width.saturating_sub(2).max(1);
    let body = if open {
        super::details::render(cell, body_width, width)
    } else {
        Vec::new()
    };
    animate(render(
        &title,
        body,
        width,
        open,
        appearance::health(cell),
        matches!(cell, HistoryCell::Reasoning(_)),
    ))
}

/// 【终端】【实时节点】实时思考和工具参数显示在链内，不再占用输入区上方的固定面板。
/// 参数: `store` 为记录，`kind` 为实时节点类型，`width` 为净宽，`open` 为展开状态；返回: 节点。
pub(super) fn render_live(
    store: &TranscriptStore,
    kind: LiveNode,
    width: usize,
    open: bool,
) -> RenderedNode {
    let elapsed = store
        .work_status_started
        .or(store.live_animation_started)
        .map(|started| started.elapsed())
        .unwrap_or_default();
    if let Some(mut node) = super::standard_node::render_live(store, &kind, width, open) {
        let icon = match kind {
            LiveNode::Thought => super::pulse::thinking_pulse_icon(elapsed).to_string(),
            LiveNode::Tool => crate::render::guide_symbols::pulse(
                crate::render::guide_symbols::tool(&store.live_tool_call.as_ref().unwrap().name),
                crate::render::activity_animation::activity_frame_at(elapsed),
            ),
        };
        super::pulse::apply_marker(&mut node.lines, &icon);
        return node;
    }
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    let title = match kind {
        LiveNode::Thought => {
            let tail = store.live_tail.as_ref().unwrap();
            let icon = super::pulse::thinking_pulse_icon(elapsed);
            format!(
                "{icon} {} · {} {} · {:.1}s",
                t("Thinking", "思考中"),
                crate::token_counter::count(&tail.source),
                t("tokens", "词元"),
                tail.reasoning_duration().unwrap_or_default().as_secs_f64()
            )
        }
        LiveNode::Tool => {
            let call = store.live_tool_call.as_ref().unwrap();
            let icon = crate::render::guide_symbols::tool(&call.name);
            format!(
                "{icon} {} · {}",
                t("Preparing tool call", "准备工具调用"),
                call.name
            )
        }
    };
    let body = if open {
        let details = match kind {
            LiveNode::Tool => crate::render::tool_view::structured_payload::format_preview(
                &store.live_tool_call.as_ref().unwrap().arguments_preview,
            ),
            LiveNode::Thought => store.activity_snapshot(&options).details,
        };
        super::aligned_wrap::wrap_aligned_block(&details, width.saturating_sub(2).max(1))
    } else {
        Vec::new()
    };
    let mut node = render(
        &title,
        body,
        width,
        open,
        ToolHealth::Pending,
        matches!(kind, LiveNode::Thought),
    );
    if matches!(kind, LiveNode::Tool) {
        let icon = crate::render::guide_symbols::tool(&store.live_tool_call.as_ref().unwrap().name);
        let marker = crate::render::guide_symbols::pulse(
            icon,
            crate::render::activity_animation::activity_frame_at(elapsed),
        );
        super::pulse::apply_marker(&mut node.lines, &marker);
    }
    node
}

/// 【终端】【节点渲染】拼接步骤标题与带层级缩进的正文，超宽时保留 ANSI 样式截断。
///
/// 参数:
/// - `title`: 单行纯文本摘要
/// - `body`: 展开的正文行列表
/// - `width`: 节点可用总宽度
/// - `open`: 是否处于展开状态
/// - `health`: 步骤执行健康状态
/// - `thinking`: 是否为思考过程
///
/// 返回:
/// - 渲染后的节点数据结构
fn render(
    title: &str,
    body: Vec<AnsiLine>,
    width: usize,
    open: bool,
    health: ToolHealth,
    thinking: bool,
) -> RenderedNode {
    // 1. 【终端】【标题生成】渲染带状态着色的单行标题
    let mut lines = vec![appearance::title(title, health, thinking, width)];
    let indent = width.saturating_sub(1).min(2);
    let available = width.saturating_sub(indent);

    // 2. 【终端】【正文缩进】为展开正文逐行追加缩进并在超宽时保留 ANSI 样式截断
    for line in body {
        let text = if crate::render::ansi_clip::ansi_visible_width(line.as_str()) > available {
            crate::render::ansi_clip::clip_ansi_to_width(line.as_str(), available, "")
        } else {
            line.as_str().to_string()
        };
        lines.push(AnsiLine::new(format!("{}{text}", " ".repeat(indent))));
    }
    RenderedNode { lines, open }
}

use super::super::cell::HistoryCell;
use super::super::fullscreen_parts::ParagraphKey;
use super::super::fullscreen_view::{FullscreenDocument, ParagraphSpan};
use super::super::line::AnsiLine;
use super::super::store::{TranscriptRenderOptions, TranscriptStore};
use super::super::tool_cell::ToolCell;
use super::appearance;
use super::node::{self, LiveNode};
use crate::llm::ChatStreamKind;
use std::collections::HashSet;
use std::ops::Range;

/// 【终端】【执行链分组】判断是否属于步骤，普通用户消息和回答不加入执行链。
/// 参数: `cell` 为源记录；返回: 是否为执行步骤。
pub(in crate::render::transcript) fn is_activity(cell: &HistoryCell) -> bool {
    matches!(
        cell,
        HistoryCell::Reasoning(_)
            | HistoryCell::Tool(_)
            | HistoryCell::Diff(_)
            | HistoryCell::Shell(_)
    )
}

/// 【终端】【执行链分组】渲染计数标题和线性节点，组内审批不能被整体折叠遮挡。
/// 参数: `store` 为源记录，`range` 为连续步骤范围，`tail` 为是否追加实时节点，`width` 为净宽，
/// `options` 为可见性偏好，`expanded` 为折叠键；返回: 分组相对坐标文档。
pub(super) fn render(
    store: &mut TranscriptStore,
    range: Range<usize>,
    tail: bool,
    width: usize,
    options: &TranscriptRenderOptions,
    expanded: &HashSet<ParagraphKey>,
) -> FullscreenDocument {
    let mut nodes = Vec::new();
    let mut tools = 0;
    let mut reads = 0;
    let mut thoughts = 0;
    let indices = range
        .clone()
        .filter(|index| node::visible(&store.cells[*index], options))
        .collect::<Vec<_>>();
    let permission = indices
        .iter()
        .any(|index| node::awaiting_permission(&store.cells[*index]));
    let mut retained_keys = Vec::new();
    let group_key = ParagraphKey::activity(range.start, true);
    let live_call = tail && store.live_tool_call.is_some() && node::tools_visible(options);
    let live_thought =
        tail && store.live_tail.as_ref().is_some_and(|tail| {
            tail.kind == ChatStreamKind::Reasoning && !tail.source.trim().is_empty()
        }) && node::thoughts_visible(options);
    let total = indices.len() + usize::from(live_thought) + usize::from(live_call);
    let single = total == 1;
    let group_open = single || permission || !expanded.contains(&group_key);
    let inset = if single {
        0
    } else {
        width.saturating_sub(1).min(2)
    };
    let node_width = width.saturating_sub(inset).max(1);
    let newer = live_call
        || live_thought
        || (tail
            && store
                .live_tail
                .as_ref()
                .is_some_and(|tail| tail.kind == ChatStreamKind::Content));
    let older = super::older_steps::OlderSteps::new(range.start, total, expanded, permission);
    let mut details = std::mem::take(&mut store.recent_details);
    let mut newer_rows = 0;
    let thought_index = store.cells.len();
    let call_index = thought_index
        + usize::from(
            store
                .live_tail
                .as_ref()
                .is_some_and(|tail| tail.kind == ChatStreamKind::Reasoning),
        );
    // 1. 【终端】【执行链分组】从末尾累计实际行数，实时与定稿节点使用相同下标
    if live_call {
        let call_name = &store.live_tool_call.as_ref().unwrap().name;
        if is_read_name(call_name) {
            reads += 1;
        } else {
            tools += 1;
        }
        let (key, node) = details.render(
            call_index,
            &mut newer_rows,
            false,
            false,
            expanded,
            |open| node::render_live(store, LiveNode::Tool, node_width, open),
        );
        retained_keys.extend([
            ParagraphKey::activity(call_index, false),
            ParagraphKey::whole(call_index),
        ]);
        if group_open {
            nodes.push((key, node, true));
        }
    }
    if live_thought {
        thoughts += 1;
        let (key, node) = details.render(
            thought_index,
            &mut newer_rows,
            false,
            false,
            expanded,
            |open| node::render_live(store, LiveNode::Thought, node_width, open),
        );
        retained_keys.extend([
            ParagraphKey::activity(thought_index, false),
            ParagraphKey::whole(thought_index),
        ]);
        if group_open && older.visible(indices.len()) {
            nodes.push((key, node, true));
        }
    }
    let (cells, timeline_cache) = (&store.cells, &mut store.timeline_cache);
    for (position, index) in indices.iter().copied().enumerate().rev() {
        let cell = &cells[index];
        if matches!(cell, HistoryCell::Reasoning(_)) {
            thoughts += 1;
        } else if is_read_cell(cell) {
            reads += 1;
        } else {
            tools += 1;
        }
        let awaiting = node::awaiting_permission(cell);
        let active = store.active_tool_index == Some(index) && !newer;
        let preview = matches!(cell, HistoryCell::Tool(ToolCell::Invocation(view)) if crate::render::tool_view::has_preview_details(&view.name));
        let mut render = |open| {
            if !active && !awaiting {
                timeline_cache.get_or_render(index, node_width, open, || {
                    node::render_cell(cell, node_width, false, open)
                })
            } else {
                node::render_cell(cell, node_width, active, open)
            }
        };
        let (key, rendered) = details.render(
            index,
            &mut newer_rows,
            awaiting,
            preview,
            expanded,
            |open| {
                if expanded.contains(&ParagraphKey::whole(index)) {
                    crate::render::render_expand::with_force_expand(|| render(open))
                } else {
                    crate::render::render_expand::with_force_collapse(|| render(open))
                }
            },
        );
        retained_keys.extend([
            ParagraphKey::activity(index, false),
            ParagraphKey::whole(index),
        ]);
        if group_open && older.visible(position) {
            nodes.push((key, rendered, !awaiting));
        }
    }
    store.recent_details = details;
    nodes.reverse();
    let mut document = FullscreenDocument::default();
    if tools + reads + thoughts == 0 {
        return document;
    }
    let key = group_key;
    let open = group_open;
    document.retained_keys.extend(retained_keys);
    // 2. 【终端】【执行链分组】标题单独可点击，不能抢占子节点的点击范围
    let counts = counts_title(tools, reads, thoughts);
    if !single {
        document.lines.push(appearance::group_title(&counts, width));
    }
    if !permission && !single {
        document.paragraphs.push(ParagraphSpan {
            key,
            start: 0,
            end: 1,
            expanded: open,
            control: 0,
        });
    }
    older.append_control(&mut document, node_width, open, inset);
    let details_open = nodes.iter().any(|(_, node, _)| node.open);
    for (key, node, interactive) in nodes {
        let start = document.lines.len();
        let reasoning = matches!(store.cells.get(key.cell), Some(HistoryCell::Reasoning(_)))
            || (live_thought && key.cell == thought_index);
        document.lines.extend(
            node.lines
                .into_iter()
                .map(|line| AnsiLine::new(format!("{}{}", " ".repeat(inset), line.as_str()))),
        );
        if reasoning && node.open {
            document
                .reasonings
                .push(super::super::fullscreen_view::ReasoningRegion {
                    key,
                    start,
                    end: document.lines.len(),
                    live: live_thought && key.cell == thought_index,
                });
        }
        if interactive {
            let preview = matches!(store.cells.get(key.cell), Some(HistoryCell::Tool(ToolCell::Invocation(view))) if crate::render::tool_view::has_preview_details(&view.name));
            document.paragraphs.push(ParagraphSpan {
                key,
                start,
                end: document.lines.len(),
                expanded: node.open && (!preview || expanded.contains(&key)),
                control: start,
            });
        }
    }
    document
        .activities
        .push(super::super::activity_region::ActivityRegion {
            key: group_key,
            start: 0,
            end: document.lines.len(),
            scrollable: !permission && details_open,
            active: live_call
                || live_thought
                || store
                    .active_tool_index
                    .is_some_and(|index| range.contains(&index)),
            footer: None,
        });
    document
}

/// 【终端】【只读判断】判断工具是否属于只读/检索操作。
///
/// 参数:
/// - `name`: 工具标识名称
///
/// 返回:
/// - 若为读取/搜索/文件检查工具返回 true，否则返回 false
fn is_read_name(name: &str) -> bool {
    let base = name.rsplit("__").next().unwrap_or(name);
    matches!(base, "read_file" | "read_image" | "read" | "grep" | "glob")
        || base.starts_with("read_")
}

/// 【终端】【单元只读判断】判断源单元是否属于只读工具调用。
///
/// 参数:
/// - `cell`: 历史单元
///
/// 返回:
/// - 是否为只读调用
fn is_read_cell(cell: &HistoryCell) -> bool {
    match cell {
        HistoryCell::Tool(crate::render::transcript::tool_cell::ToolCell::Invocation(view)) => {
            is_read_name(&view.name)
        }
        _ => false,
    }
}

/// 【终端】【分组计数】显示动作工具、只读检索与思考数量，避免零计数占用标题空间。
///
/// 参数:
/// - `tools`: 动作/命令工具调用数量
/// - `reads`: 只读检索工具调用数量
/// - `thoughts`: 思考过程数量
///
/// 返回:
/// - 英文分组标题，如 `23 tools · 20 reads · 13 thoughts`
fn counts_title(tools: usize, reads: usize, thoughts: usize) -> String {
    // 1. 【终端】【分类汇总】按工具、读取、思考的顺序组装非零项
    let mut parts = Vec::new();
    if tools > 0 {
        parts.push(format!(
            "{} {tools} {}",
            crate::render::guide_symbols::TOOL,
            crate::render::terminal_text(if tools == 1 { "tool" } else { "tools" }, "工具")
        ));
    }
    if reads > 0 {
        parts.push(format!(
            "{} {reads} {}",
            crate::render::guide_symbols::READ,
            crate::render::terminal_text(if reads == 1 { "read" } else { "reads" }, "读取")
        ));
    }
    if thoughts > 0 {
        parts.push(format!(
            "{} {thoughts} {}",
            crate::render::guide_symbols::THOUGHT,
            crate::render::terminal_text(
                if thoughts == 1 { "thought" } else { "thoughts" },
                "思考"
            )
        ));
    }

    // 2. 【终端】【标点连接】用居中点号组合多类统计
    parts.join(" · ")
}

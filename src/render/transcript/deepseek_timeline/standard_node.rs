use super::super::{
    cell::HistoryCell, line::AnsiLine, reasoning_cell, spacing, store::TranscriptStore,
    tool_cell::ToolCell,
};
use super::node::{LiveNode, RenderedNode};
use crate::render::{
    render_expand, render_width, status_style::ToolHealth, terminal_text as t, tool_event_line,
    ReasoningDisplayMode, ToolCallDisplayMode,
};

/// 【终端】【普通节点】把普通命令和思考视图放入执行链，不添加第二份标题或 JSON 正文。
/// 参数: `cell` 为单元，`width` 为净宽，`open` 为展开状态；返回: 普通样式节点，其他单元为空。
pub(super) fn render_cell(cell: &HistoryCell, width: usize, open: bool) -> Option<RenderedNode> {
    let supported = matches!(cell, HistoryCell::Reasoning(_) | HistoryCell::Shell(_))
        || matches!(cell, HistoryCell::Tool(ToolCell::Invocation(view)) if matches!(view.name.as_str(), "run_command" | "todo" | "background_notice" | "conversation_notice") || crate::tools::computer_use::is_tool(&view.name));
    if !supported {
        return None;
    }
    let lines = render_width::with_render_width(width, || {
        if open {
            let options = super::super::store::TranscriptRenderOptions {
                reasoning_mode: ReasoningDisplayMode::Full,
                tool_call_mode: ToolCallDisplayMode::Full,
            };
            render_expand::with_force_expand(|| cell.display_lines_framed(width, &options, 0))
        } else {
            render_expand::with_force_collapse(|| {
                let text = match cell {
                    HistoryCell::Reasoning(cell) => {
                        reasoning_cell::render(cell, ReasoningDisplayMode::Summary)
                    }
                    HistoryCell::Tool(ToolCell::Invocation(view))
                        if matches!(
                            view.name.as_str(),
                            "todo" | "background_notice" | "conversation_notice"
                        ) || crate::tools::computer_use::is_tool(&view.name) =>
                    {
                        crate::render::tool_view::render(view, ToolCallDisplayMode::Summary)
                    }
                    HistoryCell::Tool(ToolCell::Invocation(view)) => {
                        let health = super::appearance::health(cell);
                        let action = if super::node::awaiting_permission(cell) {
                            "Awaiting approval"
                        } else if health == ToolHealth::Err {
                            "Failed · Ran"
                        } else {
                            tool_event_line::tool_verb(
                                "run_command",
                                tool_event_line::ToolVerbTense::from_done(view.outcome.is_some()),
                            )
                        };
                        crate::render::command_output::render_command_block_with_action(
                            &view.arguments,
                            action,
                            health,
                        )
                    }
                    HistoryCell::Shell(cell) => {
                        let failed = cell.exit_code.filter(|code| *code != 0);
                        let action = failed.map_or_else(
                            || "You ran".to_string(),
                            |code| format!("Failed (exit {code}) · You ran"),
                        );
                        crate::render::command_output::render_command_block_with_action(
                            &serde_json::json!({"command":cell.command}).to_string(),
                            &action,
                            if failed.is_some() {
                                ToolHealth::Err
                            } else {
                                ToolHealth::Ok
                            },
                        )
                    }
                    _ => String::new(),
                };
                AnsiLine::wrap_block(&text, width)
            })
        }
    });
    Some(finish(lines, width, open))
}

/// 【终端】【流式普通节点】参数准备和实时思考复用普通视图，与后续执行保持同一结构。
/// 参数: `store` 为记录，`kind` 为实时类型，`width` 为净宽，`open` 为展开状态；返回: 普通样式节点或空值。
pub(super) fn render_live(
    store: &TranscriptStore,
    kind: &LiveNode,
    width: usize,
    open: bool,
) -> Option<RenderedNode> {
    let text = render_width::with_render_width(width, || match kind {
        LiveNode::Thought => store.live_tail.as_ref().map(|tail| {
            reasoning_cell::render_live_body(
                &tail.source,
                if open {
                    ReasoningDisplayMode::Full
                } else {
                    ReasoningDisplayMode::Summary
                },
                open,
            )
        }),
        LiveNode::Tool => store
            .live_tool_call
            .as_ref()
            .filter(|call| call.name == "run_command")
            .map(|call| {
                let command =
                    tool_event_line::lenient_string_field(&call.arguments_preview, "command")
                        .unwrap_or_default();
                let arguments = serde_json::json!({"command":command}).to_string();
                let render = || {
                    crate::render::command_output::render_command_block_with_action(
                        &arguments,
                        t("Preparing", "准备中"),
                        ToolHealth::Pending,
                    )
                };
                if open {
                    render_expand::with_force_expand(render)
                } else {
                    render_expand::with_force_collapse(render)
                }
            }),
    })?;
    Some(finish(AnsiLine::wrap_block(&text, width), width, open))
}

/// 【终端】【节点边界】移除留白并在超宽时保留 ANSI 样式截断。
///
/// 参数:
/// - `lines`: 待处理终端行
/// - `width`: 允许的最大列宽
/// - `open`: 是否展开
///
/// 返回:
/// - 满足列宽边界的渲染节点
fn finish(mut lines: Vec<AnsiLine>, width: usize, open: bool) -> RenderedNode {
    // 1. 【终端】【留白清理】去除收尾无效空行
    spacing::trim_leading_visual_blanks(&mut lines);
    spacing::trim_trailing_visual_blanks(&mut lines);

    // 2. 【终端】【折叠裁切】非展开状态只保留首行摘要
    if !open {
        lines.truncate(1);
    }

    // 3. 【终端】【安全截断】逐行检查列宽并使用 ANSI 安全函数截断
    for line in &mut lines {
        if crate::render::ansi_clip::ansi_visible_width(line.as_str()) > width {
            *line = AnsiLine::new(crate::render::ansi_clip::clip_ansi_to_width(line.as_str(), width, ""));
        }
    }
    RenderedNode { lines, open }
}

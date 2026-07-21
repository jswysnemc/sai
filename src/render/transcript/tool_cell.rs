use super::subagent_cell::{self, SubagentCell};
use crate::render::edit_diff::render_edit_file_diff;
use crate::render::stream_text::is_file_edit_tool;
use crate::render::terminal_text as t;
use crate::render::tool_event_line::tool_event_text;
use crate::render::tool_view::{self, ToolView};
use crate::render::ToolCallDisplayMode;

/// REPL 工具历史单元。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ToolCell {
    Invocation(ToolView),
    Subagent(SubagentCell),
    CompactionStarted {
        turn_count: usize,
        model: String,
    },
    CompactionFinished {
        applied: bool,
        message: Option<String>,
        detail: Option<String>,
        /// 成功应用时的压缩摘要正文。
        summary: Option<String>,
    },
}

/// 渲染工具历史单元。
///
/// 参数:
/// - `cell`: 工具历史数据
/// - `mode`: 工具展示模式
///
/// 返回:
/// - ANSI 工具视图文本
pub(crate) fn render(cell: &ToolCell, mode: ToolCallDisplayMode) -> String {
    match cell {
        ToolCell::Invocation(view) => tool_view::render(view, mode),
        ToolCell::Subagent(cell) => subagent_cell::render(cell, mode),
        ToolCell::CompactionStarted { turn_count, model } => tool_event_text(
            &format!(
                "{}×{turn_count} · {model}",
                t("compact context", "压缩上下文")
            ),
            "run",
        ),
        ToolCell::CompactionFinished {
            applied,
            message,
            detail,
            summary,
        } => {
            let mut lines = vec![tool_event_text(
                t("compact context", "压缩上下文"),
                if *applied { "ok" } else { "skip" },
            )];
            if let Some(message) = message
                .as_ref()
                .map(String::as_str)
                .filter(|value| !value.is_empty())
            {
                lines.push(message.to_string());
            }
            if let Some(detail) = detail
                .as_ref()
                .map(String::as_str)
                .filter(|value| !value.is_empty() && message.as_deref() != Some(*value))
            {
                lines.push(detail.to_string());
            }
            if *applied {
                if let Some(summary) = summary
                    .as_ref()
                    .map(String::as_str)
                    .filter(|value| !value.trim().is_empty())
                {
                    lines.push(summary.trim().to_string());
                }
            }
            lines.join("\n")
        }
    }
}

/// 渲染尚未接收完整参数的工具调用。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments_preview`: 当前参数预览
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 可重绘的临时工具视图
pub(crate) fn render_live_call(
    name: &str,
    arguments_preview: &str,
    mode: ToolCallDisplayMode,
) -> String {
    // 1. 编辑类工具在参数流阶段优先渲染 diff，与 CLI 流式预览一致
    if is_file_edit_tool(name) {
        if let Some(diff) = render_edit_file_diff(arguments_preview) {
            return diff.trim_end().to_string();
        }
    }
    tool_view::render(
        &ToolView::preparing(name.to_string(), arguments_preview.to_string()),
        mode,
    )
}

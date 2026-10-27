use super::super::{cell::HistoryCell, line::AnsiLine, tool_cell::ToolCell};
use crate::render::status_style::ToolHealth;

/// 【终端】【节点着色】从步骤结果推导状态，不把失败或尚未执行的步骤标为成功。
/// 参数: `cell` 为步骤；返回: 状态颜色对应的语义。
pub(super) fn health(cell: &HistoryCell) -> ToolHealth {
    match cell {
        HistoryCell::Tool(ToolCell::Invocation(view)) => match &view.outcome {
            Some(outcome) => {
                let ok = if view.name == "run_command" {
                    crate::render::command_result_block::command_result_streams(&outcome.output)
                        .map(|(success, _, _)| success)
                        .unwrap_or(outcome.ok)
                } else {
                    outcome.ok
                };
                if ok {
                    ToolHealth::Ok
                } else {
                    ToolHealth::Err
                }
            }
            None => ToolHealth::Pending,
        },
        HistoryCell::Diff(cell) => cell.health(),
        HistoryCell::Shell(cell) => match cell.exit_code {
            Some(0) => ToolHealth::Ok,
            Some(_) => ToolHealth::Err,
            None => ToolHealth::Pending,
        },
        HistoryCell::Tool(ToolCell::Subagent(cell)) => {
            ToolHealth::from_status(cell.overview().status)
        }
        HistoryCell::Tool(ToolCell::CompactionStarted { .. }) => ToolHealth::Pending,
        HistoryCell::Tool(ToolCell::CompactionFinished { .. }) => ToolHealth::Ok,
        _ => ToolHealth::Neutral,
    }
}

/// 【终端】【节点着色】为动作标记添加语义色，统计和耗时保持弱化，不污染后续正文。
/// 参数: `title` 为纯文本摘要，`health` 为状态，`thinking` 为是否思考，`width` 为宽度；返回: 标题行。
pub(super) fn title(title: &str, health: ToolHealth, thinking: bool, width: usize) -> AnsiLine {
    let plain = crate::render::ansi_clip::strip_ansi(title)
        .replace(['\n', '\r', '\t'], " ");
    let clipped = crate::render::clip_to_width(&plain, width, "");
    let (action, metadata) = clipped.split_once(" · ").unwrap_or((&clipped, ""));
    let color = match health {
        ToolHealth::Ok => "\x1b[32m",
        ToolHealth::Err => "\x1b[31m",
        _ if thinking => "\x1b[36m",
        ToolHealth::Pending => "\x1b[33m",
        ToolHealth::Neutral => "\x1b[2m",
    };
    let (icon, label) = action.split_once(' ').unwrap_or((action, ""));
    let mut rendered = format!("{color}{icon}\x1b[0m");
    if !label.is_empty() {
        rendered.push_str(&format!(" {label}"));
    }
    if !metadata.is_empty() {
        rendered.push_str(&format!("\x1b[2m · {metadata}\x1b[0m"));
    }
    AnsiLine::new(format!("{rendered}\x1b[0m"))
}

/// 【终端】【分组着色】突出折叠标记与计数，避免给整块正文添加背景。
/// 参数: `text` 为分组标题，`width` 为净宽；返回: 带颜色的定宽标题。
pub(super) fn group_title(text: &str, width: usize) -> AnsiLine {
    AnsiLine::new(format!(
        "\x1b[36m{}\x1b[0m",
        crate::render::clip_to_width(text, width, "")
    ))
}

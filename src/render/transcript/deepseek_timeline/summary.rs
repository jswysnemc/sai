use super::super::{cell::HistoryCell, tool_cell::ToolCell};
use super::{appearance, file_summary, tool_summary};
use crate::render::status_style::ToolHealth;
use crate::render::terminal_text as t;
use crate::render::tool_event_line::{tool_event_label_tense, ToolVerbTense};

/// 【终端】【步骤摘要】优先展示失败与审批状态，文件摘要按可用宽度保留关键内容。
/// 参数: `cell` 为步骤，`width` 为包含标记的显示列数，`approval` 为审批状态；返回: 单行纯文本标题。
pub(super) fn for_cell(cell: &HistoryCell, width: usize, approval: bool) -> String {
    let health = appearance::health(cell);
    match cell {
        HistoryCell::Reasoning(cell) => {
            let mut title = format!(
                "◇ {} · {} {}",
                t("Reasoning complete", "已思考"),
                crate::token_counter::count(&cell.source),
                t("tokens", "词元")
            );
            if let Some(duration) = cell.duration {
                title.push_str(&format!(" · {:.1}s", duration.as_secs_f64()));
            }
            title
        }
        HistoryCell::Tool(ToolCell::Invocation(view)) => {
            let elapsed = view
                .elapsed()
                .map(|duration| {
                    if duration.as_millis() < 1000 {
                        format!(" · {}ms", duration.as_millis())
                    } else {
                        format!(" · {:.1}s", duration.as_secs_f64())
                    }
                })
                .unwrap_or_default();
            let label = file_summary::label(view, &elapsed, width.saturating_sub(2), approval)
                .or_else(|| tool_summary::label(view, &elapsed, width.saturating_sub(2), approval))
                .unwrap_or_else(|| {
                    let label = tool_event_label_tense(
                        &view.name,
                        Some(&view.arguments),
                        ToolVerbTense::from_done(view.outcome.is_some()),
                    );
                    with_status(format!("{label}{elapsed}"), health, approval)
                });
            format!("{} {label}", tool_icon(&view.name))
        }
        HistoryCell::Diff(cell) => {
            format!(
                "▤ {}",
                with_status(cell.timeline_summary(), health, approval)
            )
        }
        HistoryCell::Shell(cell) => {
            let command = cell.command.replace('\n', " ");
            if let Some(code) = cell.exit_code.filter(|code| *code != 0) {
                format!("$ {} (exit {code}) · {command}", t("Failed", "失败"))
            } else {
                format!("$ {command}")
            }
        }
        HistoryCell::Tool(ToolCell::Subagent(cell)) => {
            format!("◇ {}", with_status(cell.overview().label, health, false))
        }
        HistoryCell::Tool(ToolCell::CompactionStarted { .. }) => {
            format!("◇ {}", t("Compacting context", "压缩上下文"))
        }
        HistoryCell::Tool(ToolCell::CompactionFinished { .. }) => {
            format!("◇ {}", t("Compacted context", "已压缩上下文"))
        }
        _ => String::new(),
    }
}

/// 【终端】【状态摘要】把结果放在长参数前面，保证失败和待审批状态不被尾部裁剪。
/// 参数: `label` 为动作，`health` 为执行结果，`approval` 为是否等待审批；返回: 状态优先的摘要。
fn with_status(label: String, health: ToolHealth, approval: bool) -> String {
    if health == ToolHealth::Err {
        format!("{} · {label}", t("Failed", "失败"))
    } else if approval {
        format!("{} · {label}", t("Awaiting approval", "等待审批"))
    } else {
        label
    }
}

/// 【终端】【工具标记】使用单色文本符号区分文件、命令和其他工具。
/// 参数: `name` 为工具名称；返回: 单列标记。
pub(super) fn tool_icon(name: &str) -> &'static str {
    let name = name.rsplit("__").next().unwrap_or(name);
    if matches!(name, "run_command" | "background_command") {
        "$"
    } else if name == "read" || name.starts_with("read_") {
        crate::render::style::TOOL_BULLET
    } else if name.contains("file") || name == "str_replace" {
        "▤"
    } else {
        "·"
    }
}

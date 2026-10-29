use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【后台清理】显示已移除任务数量、标识和日志处理范围。
/// 参数: 工具视图及显示模式；返回: 成功清理卡片，错误或其他动作交给通用视图。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "background_command" {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    if value["ok"] == false {
        return None;
    }
    let removed = value.get("removed")?.as_array()?;
    let args: Value = serde_json::from_str(&view.arguments).unwrap_or_default();
    let count = removed.len();
    let title = format!(
        "Cleared {count} {} · background command records",
        if count == 1 { "task" } else { "tasks" }
    );
    let mut body = removed
        .iter()
        .filter_map(Value::as_str)
        .map(|id| format!("Task: {}", crate::render::ansi_clip::strip_ansi(id)))
        .collect::<Vec<_>>();
    if count == 0 {
        body.push("No finished tasks to clear.".into());
    }
    if let Some(remaining) = value["remaining"].as_u64() {
        body.push(format!("{remaining} tasks remain tracked"));
    }
    if count > 0 {
        if let Some(remove_logs) = args["remove_logs"].as_bool() {
            body.push(
                if remove_logs {
                    "Logs: removal requested"
                } else {
                    "Logs: retained"
                }
                .into(),
            );
        }
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        ToolHealth::Ok,
        &body.join("\n"),
    ))
}

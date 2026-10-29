use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【目标结果】仅展示目标、状态、最近进度及用户显式设置的预算。
/// 参数: 工具视图和显示模式；返回: 目标专用视图，其他工具返回空。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if !matches!(
        view.name.as_str(),
        "create_goal" | "get_goal" | "update_goal"
    ) {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    let goal = value.get("goal").unwrap_or(&value);
    if goal.is_null() {
        return Some(result_card::render(
            view,
            mode,
            "No active goal",
            ToolHealth::Neutral,
            "",
        ));
    }
    let objective = result_card::text(goal, "objective")?;
    let status = goal["status"]
        .as_str()
        .unwrap_or("active")
        .replace('_', " ");
    let title = format!("Goal {status} · {}", result_card::short(&objective, 64));
    let health = if matches!(
        status.as_str(),
        "blocked" | "budget limited" | "usage limited"
    ) {
        ToolHealth::Err
    } else if status == "complete" {
        ToolHealth::Ok
    } else {
        ToolHealth::Neutral
    };
    let mut body = Vec::new();
    if objective != result_card::short(&objective, 64) {
        body.push(objective);
    }
    // 1. 【终端】【目标进度】只保留最近一条进度，不复述完整更新历史
    if let Some(note) = goal["updates"]
        .as_array()
        .and_then(|items| items.iter().rev().find(|item| item["kind"] == "progress"))
        .and_then(|item| result_card::text(item, "message"))
    {
        body.push(note);
    }
    if let Some(budget) = goal["token_budget"].as_u64().filter(|budget| *budget > 0) {
        let used = goal["tokens_used"].as_u64().unwrap_or(0);
        body.push(format!("Budget: {used} / {budget} tokens"));
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        health,
        &body.join("\n"),
    ))
}

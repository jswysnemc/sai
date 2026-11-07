use super::{result_card, ToolView};
use crate::i18n::text as t;
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【目标结果】按动作展示目标、状态、最近进度及用户显式设置的预算。
/// 参数: `view` 为工具视图，`mode` 为显示模式；返回: 目标专用视图，无法识别时交回默认渲染。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    let action = match view.name.as_str() {
        "create_goal" => t("Create goal", "创建目标"),
        "get_goal" => t("View goal", "查看目标"),
        "update_goal" => t("Update goal", "更新目标"),
        _ => return None,
    };
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    let goal = value.get("goal").unwrap_or(&value);
    if goal.is_null() {
        return Some(result_card::render(
            view,
            mode,
            t("No active goal", "尚未设置目标"),
            ToolHealth::Neutral,
            "",
        ));
    }
    let objective = result_card::text(goal, "objective")?;
    let (status, health) = status_label(goal["status"].as_str().unwrap_or("active"));
    let title = format!(
        "{action} · {status} · {}",
        result_card::short(&objective, 48)
    );
    let mut body = Vec::new();
    if mode == ToolCallDisplayMode::Full {
        body.push(section(t("Objective", "目标"), &objective));
    }
    // 1. 【终端】【目标进度】跳过状态、计费及空进度记录，只展示最近一条可读进度
    if let Some(note) = latest_progress(goal) {
        if mode == ToolCallDisplayMode::Summary {
            body.push(format!(
                "{}：{}",
                t("Latest progress", "最近进度"),
                result_card::short(&note, 64)
            ));
        } else {
            body.push(section(t("Latest progress", "最近进度"), &note));
        }
    }
    if let Some(budget) = goal["token_budget"].as_u64().filter(|budget| *budget > 0) {
        let used = goal["tokens_used"].as_u64().unwrap_or(0);
        body.push(format!("{}：{used} / {budget} tokens", t("Budget", "预算")));
    }
    // 2. 【终端】【目标摘要】短进度和预算复用卡片正文布局，避免摘要模式再次隐藏进度
    Some(result_card::render(
        view,
        ToolCallDisplayMode::Full,
        &title,
        health,
        &body.join("\n"),
    ))
}

/// 【终端】【目标状态】将持久化状态转换为中文，保留未知状态的安全短文本。
/// 参数: `status` 为原始状态；返回: 中文状态标签与卡片健康状态。
fn status_label(status: &str) -> (String, ToolHealth) {
    let (label, health) = match status {
        "active" => (t("Active", "进行中"), ToolHealth::Neutral),
        "paused" => (t("Paused", "已暂停"), ToolHealth::Neutral),
        "blocked" => (t("Blocked", "受阻"), ToolHealth::Err),
        "usage_limited" => (t("Usage limited", "用量受限"), ToolHealth::Err),
        "budget_limited" => (t("Budget limited", "预算受限"), ToolHealth::Err),
        "complete" | "completed" => (t("Complete", "已完成"), ToolHealth::Ok),
        _ => {
            return (
                format!(
                    "{}（{}）",
                    t("Unknown status", "未知状态"),
                    result_card::short(status, 20)
                ),
                ToolHealth::Neutral,
            );
        }
    };
    (label.to_string(), health)
}

/// 【终端】【最近进度】从更新历史末尾寻找最近一条非空进度，不泄露其他历史字段。
/// 参数: `goal` 为目标结果；返回: 已清理控制符的进度正文，无有效记录时返回空。
fn latest_progress(goal: &Value) -> Option<String> {
    goal["updates"].as_array()?.iter().rev().find_map(|item| {
        if item["kind"] == "progress" {
            result_card::text(item, "message")
        } else {
            None
        }
    })
}

/// 【终端】【目标分层】为多行正文添加独立标签和统一缩进，保留原始段落。
/// 参数: `label` 为分区标签，`text` 为已清理的正文；返回: 可供共用卡片换行的分区文本。
fn section(label: &str, text: &str) -> String {
    format!("{label}：\n  {}", text.replace('\n', "\n  "))
}

#[cfg(test)]
#[path = "goal_tests.rs"]
mod tests;

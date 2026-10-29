use super::{result_card, ToolView};
use crate::render::{
    status_style::ToolHealth,
    tool_event_line::{tool_event_label_tense, ToolVerbTense},
    ToolCallDisplayMode,
};
use serde_json::Value;

/// 【终端】【子任务标题】优先读取结果描述，历史缺少参数时也保留任务名称。
/// 参数: 参数、可选结果及完成状态；返回: 带动作的任务标题。
pub(crate) fn title(arguments: &str, output: Option<&str>, done: bool) -> String {
    let mut args: Value = serde_json::from_str(arguments).unwrap_or_else(|_| serde_json::json!({}));
    let result: Value = output
        .and_then(|output| serde_json::from_str(output).ok())
        .unwrap_or_default();
    let description = result_card::text(&result["subagent"], "description")
        .or_else(|| result_card::text(&args, "description"))
        .or_else(|| result_card::text(&args, "prompt"));
    if let Some(description) = description {
        if let Some(fields) = args.as_object_mut() {
            let description = Value::String(result_card::short(&description, 64));
            fields.insert("description".into(), description.clone());
            fields.insert("subagent_id".into(), description);
        }
    }
    let mut title = tool_event_label_tense(
        "subagent",
        Some(&args.to_string()),
        ToolVerbTense::from_done(done),
    );
    if let Some(items) = result["subagents"].as_array() {
        title = format!("Listed {} subagents", items.len());
    }
    title
}

/// 【终端】【子任务结果】展示描述、状态与结论，隐藏内部标识、时间戳和运行期快照。
/// 参数: 工具视图和模式；返回: 子任务结果视图。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "subagent" {
        return None;
    }
    let outcome = view.outcome.as_ref()?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    let task = value.get("subagent").filter(|value| value.is_object());
    let tasks = value["subagents"].as_array();
    if task.is_none() && tasks.is_none() {
        return None;
    }
    let mut title = title(&view.arguments, Some(&outcome.output), true);
    let mut body = Vec::new();
    let mut health = if outcome.ok {
        ToolHealth::Ok
    } else {
        ToolHealth::Err
    };
    if let Some(task) = task {
        let status = task["status"].as_str().unwrap_or("unknown");
        title.push_str(&format!(" · {status}"));
        if matches!(status, "failed" | "interrupted") {
            health = ToolHealth::Err;
        } else if matches!(status, "running" | "idle" | "cancelled") {
            health = ToolHealth::Neutral;
        }
        if let Some(result) = result_card::text(task, "result") {
            body.push(result_card::markdown(&result));
        }
        if let Some(error) = result_card::text(task, "error") {
            body.push(error);
        }
        if body.is_empty() && status == "running" {
            if let Some(phase) = result_card::text(task, "phase") {
                body.push(phase);
            }
        }
    }
    for task in tasks.into_iter().flatten() {
        let description =
            result_card::text(task, "description").unwrap_or_else(|| "Untitled task".into());
        body.push(format!(
            "{} · {}",
            result_card::short(&description, 80),
            task["status"].as_str().unwrap_or("unknown")
        ));
    }
    if value["truncated"] == true {
        body.push(
            "Result truncated; more details are available in the subagent conversation.".into(),
        );
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        health,
        &body.join("\n"),
    ))
}

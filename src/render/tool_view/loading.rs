use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【能力结果】只显示资源名称和加载状态，定义、文档与模型指令始终留在协议中。
/// 参数: 工具视图和模式；返回: 加载或能力申请结果。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if !matches!(view.name.as_str(), "load" | "request_capability") {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    if value["ok"] == false {
        return None;
    }
    if !value["tools"].is_array() && !value["skills"].is_array() {
        return None;
    }
    let mut names = Vec::new();
    let mut body = Vec::new();
    for (key, kind) in [("tools", "Tool"), ("skills", "Skill")] {
        for item in value[key].as_array().into_iter().flatten() {
            let Some(name) = item
                .as_str()
                .map(str::to_string)
                .or_else(|| result_card::text(item, "name"))
            else {
                continue;
            };
            let status = match item["status"].as_str() {
                Some("already_loaded") => "already loaded",
                Some("loaded") => "loaded",
                _ if key == "tools" || view.name == "load" => "ready",
                _ => "available",
            };
            names.push(name.clone());
            body.push(format!("{kind} {name} · {status}"));
        }
    }
    let title = if names.is_empty() {
        "No matching capability".into()
    } else {
        format!(
            "{} · {}",
            if view.name == "load" {
                "Loaded"
            } else {
                "Capabilities ready"
            },
            result_card::short(&names.join(", "), 64)
        )
    };
    Some(result_card::render(
        view,
        mode,
        &title,
        ToolHealth::Ok,
        &body.join("\n"),
    ))
}

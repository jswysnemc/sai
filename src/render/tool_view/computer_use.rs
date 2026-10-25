//! 桌面工具展示动作、目标及可读的控件状态，不直接展示协议 JSON。

use super::{structured_payload, ToolView};
use crate::render::{
    status_style::ToolHealth, tool_event_line::tool_status_line, ToolCallDisplayMode,
};
use serde_json::Value;

/// 【终端】【桌面工具】统一渲染原有动作入口与分动作入口，摘要与详情沿用同一标题。
/// 参数: `view` 为工具生命周期，`mode` 为展示模式；返回: 桌面工具的 ANSI 文本，其他工具返回空。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if !crate::tools::computer_use::is_tool(&view.name) {
        return None;
    }
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    // 1. 【终端】【动作标题】统一入口与命名入口复用动作语义和生命周期状态
    let args = serde_json::from_str::<Value>(&view.arguments).unwrap_or_default();
    let action = args["action"]
        .as_str()
        .or_else(|| crate::tools::computer_use::canonical_action(&view.name))
        .unwrap_or("unknown");
    let done = view.outcome.is_some();
    let (present, past) = match action {
        "status" => (
            "Inspecting desktop configuration",
            "Inspected desktop configuration",
        ),
        "reset" => ("Closing desktop session", "Closed desktop session"),
        "list_apps" => ("Listing applications", "Listed applications"),
        "list_windows" => ("Listing windows", "Listed windows"),
        "get_app_state" if view.name == "computer_ui" || args["include_screenshot"] == false => {
            ("Inspecting accessibility", "Inspected accessibility")
        }
        "get_app_state" => ("Capturing window", "Captured window"),
        "click" => ("Clicking control", "Clicked control"),
        "drag" => ("Dragging in window", "Dragged in window"),
        "scroll" => ("Scrolling window", "Scrolled window"),
        "type_text" => ("Typing text", "Typed text"),
        "press_key" => ("Sending key", "Sent key"),
        "set_value" => ("Setting control value", "Set control value"),
        "invoke" => ("Invoking control", "Invoked control"),
        "show_menu" => ("Opening control menu", "Opened control menu"),
        "get_value" => ("Reading control value", "Read control value"),
        "get_text" => ("Reading control text", "Read control text"),
        "locate_text" => ("Locating controls", "Located controls"),
        _ => ("Using desktop", "Completed desktop action"),
    };
    let health = match &view.outcome {
        Some(outcome) if outcome.ok => ToolHealth::Ok,
        Some(_) => ToolHealth::Err,
        None => ToolHealth::Pending,
    };
    let mut label = if done { past } else { present }.to_owned();
    if health == ToolHealth::Err {
        label = format!("Failed · {present}");
    } else if view
        .permission
        .as_ref()
        .is_some_and(|permission| permission.decision.is_none())
    {
        label = format!("Awaiting approval · {label}");
    }
    if let Some(app) = args["app"].as_str() {
        label.push_str(&format!(" · app {}", app.replace(['\n', '\r', '\t'], " ")));
    }
    if let Some(window) = args.get("window_id") {
        label.push_str(&format!(" · window {window}"));
    }
    // 2. 【终端】【详情展示】隐藏重复目标与快照证明，仅展开可读参数和操作结果
    let mut output = tool_status_line(&label, "", health);
    if mode == ToolCallDisplayMode::Full {
        let mut details = args.clone();
        if let Some(fields) = details.as_object_mut() {
            for field in [
                "action",
                "app",
                "window_id",
                "snapshot_id",
                "from_frame",
                "include_screenshot",
            ] {
                fields.remove(field);
            }
            if !fields.is_empty() {
                output.push_str(&format!("\n{}", structured_payload::format_value(&details)));
            }
        }
        if let Some(outcome) = &view.outcome {
            let body = result_body(&outcome.output);
            if !body.trim().is_empty() {
                output.push_str(&format!("\n{body}"));
            }
        }
        if let Some(progress) = &view.progress {
            output.push_str(&format!("\n{}", structured_payload::format(progress)));
        }
    }
    output.push_str(&super::formatter::render_permission(
        view.permission.as_ref(),
    ));
    Some(output)
}

/// 【终端】【窗口状态】解包状态区域中的逐行数据，保留控件与截图信息，隐藏重复目标及模型提示。
/// 参数: `payload` 为桌面工具结果；返回: 不含 JSON 容器和协议包装的语义化正文。
fn result_body(payload: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(payload) {
        return structured_payload::format_value(&value);
    }
    payload
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if matches!(
                line,
                "<app_state>"
                    | "</app_state>"
                    | "Computer Use state"
                    | "Computer Use accessibility result"
            ) || line.starts_with("app: ")
                || line.starts_with("window_id: ")
                || line.starts_with("snapshot_id: ")
                || line.starts_with("Each input consumes")
                || line.starts_with("Call get_app_state for the updated state")
            {
                return None;
            }
            if let Some(frame) = line.strip_prefix("frame_id: ") {
                return Some(format!(
                    "Frame: {}",
                    crate::render::clip_to_width(frame, 8, "")
                ));
            }
            if let Ok(element) = serde_json::from_str::<Value>(line) {
                if let (Some(handle), Some(role)) =
                    (element["element_index"].as_str(), element["role"].as_str())
                {
                    let depth = element["depth"].as_u64().unwrap_or(0).min(8) as usize;
                    let label = element["label"]
                        .as_str()
                        .unwrap_or("")
                        .replace(['\n', '\r', '\t'], " ");
                    let mut row = format!("{}• {role} {label} [{handle}]", " ".repeat(depth * 2));
                    if let Some(value) = element
                        .get("value")
                        .filter(|value| !value.is_null() && *value != "")
                    {
                        row.push_str(&format!(
                            " · {}",
                            structured_payload::format_value(value).replace('\n', " ")
                        ));
                    }
                    if element["enabled"] == false {
                        row.push_str(" · disabled");
                    }
                    if element["selected"] == true {
                        row.push_str(" · selected");
                    }
                    if element["truncated"] == true {
                        row.push_str(" · truncated");
                    }
                    return Some(row);
                }
            }
            Some(structured_payload::format(line))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

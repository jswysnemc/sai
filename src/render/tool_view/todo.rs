use super::model::ToolView;
use crate::render::tool_event_line::{tool_event_label, tool_event_text};
use crate::render::ToolCallDisplayMode;
use serde::Deserialize;

/// TODO 工具返回的待办条目。
#[derive(Deserialize)]
struct TodoItemView {
    text: String,
    status: String,
}

/// TODO 工具的结构化返回。
#[derive(Deserialize)]
struct TodoResultView {
    items: Vec<TodoItemView>,
}

/// 渲染 TODO 工具的结构化清单。
///
/// 参数:
/// - `view`: TODO 工具生命周期
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 可识别时返回待办清单文本
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    // 运行中但尚无结果：展示操作意图
    if view.outcome.is_none() {
        let label = tool_event_label("todo", Some(&view.arguments));
        let status = if view.arguments.trim().is_empty() {
            "run"
        } else {
            "run"
        };
        return Some(tool_event_text(&label, status));
    }
    let outcome = view.outcome.as_ref()?;
    let result = serde_json::from_str::<TodoResultView>(&outcome.output).ok()?;
    let label = tool_event_label("todo", Some(&view.arguments));
    let mut output = tool_event_text(&label, if outcome.ok { "ok" } else { "err" });

    let total = result.items.len();
    let completed = result
        .items
        .iter()
        .filter(|item| item.status == "completed")
        .count();
    let in_progress = result
        .items
        .iter()
        .filter(|item| item.status == "in_progress")
        .count();
    let cancelled = result
        .items
        .iter()
        .filter(|item| item.status == "cancelled")
        .count();
    let pending = total.saturating_sub(completed + in_progress + cancelled);

    // 摘要行：进度与状态统计
    output.push_str(&format!("\n\x1b[2m  {}/{} done", completed, total));
    if in_progress > 0 {
        output.push_str(&format!(" · {in_progress} active"));
    }
    if pending > 0 {
        output.push_str(&format!(" · {pending} pending"));
    }
    if cancelled > 0 {
        output.push_str(&format!(" · {cancelled} cancelled"));
    }
    output.push_str("\x1b[0m");

    // 条目：当前进行中优先，其次待办，再完成/取消
    let mut items = result.items;
    items.sort_by_key(|item| status_rank(&item.status));

    for (index, item) in items.iter().enumerate() {
        let marker = status_marker(&item.status);
        let colored = colorize_item(&item.status, &item.text);
        let connector = if index + 1 == items.len() {
            "└─"
        } else {
            "├─"
        };
        output.push_str(&format!("\n  \x1b[2m{connector}\x1b[0m {marker} {colored}"));
    }
    Some(output)
}

/// 状态排序：进行中 > 待办 > 完成 > 取消。
fn status_rank(status: &str) -> u8 {
    match status {
        "in_progress" => 0,
        "pending" | "todo" => 1,
        "completed" => 2,
        "cancelled" => 3,
        _ => 4,
    }
}

/// 返回待办状态的纯文本/符号标记。
///
/// 参数:
/// - `status`: 待办状态
///
/// 返回:
/// - 状态标记
fn status_marker(status: &str) -> &'static str {
    match status {
        "completed" => "\x1b[32m✓\x1b[0m",
        "in_progress" => "\x1b[36m›\x1b[0m",
        "cancelled" => "\x1b[2m×\x1b[0m",
        _ => "\x1b[2m○\x1b[0m",
    }
}

/// 按状态着色待办文本。
///
/// 参数:
/// - `status`: 状态
/// - `text`: 原文
///
/// 返回:
/// - 带 ANSI 的文本
fn colorize_item(status: &str, text: &str) -> String {
    match status {
        "completed" => format!("\x1b[2m{text}\x1b[0m"),
        "in_progress" => format!("\x1b[1m\x1b[36m{text}\x1b[0m"),
        "cancelled" => format!("\x1b[2m\x1b[9m{text}\x1b[0m"),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_marker_covers_common_states() {
        assert!(status_marker("completed").contains("✓"));
        assert!(status_marker("in_progress").contains("›"));
        assert!(status_marker("pending").contains("○"));
    }
}

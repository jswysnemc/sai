use super::model::ToolView;
use crate::render::todo_style::{colorize_item, status_marker, status_rank};
use crate::render::tool_event_line::{tool_event_label_tense, tool_event_text, ToolVerbTense};
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
    #[serde(default)]
    changed: Vec<TodoItemView>,
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
        let label =
            tool_event_label_tense("todo", Some(&view.arguments), ToolVerbTense::Progressive);
        return Some(tool_event_text(&label, "run"));
    }
    let outcome = view.outcome.as_ref()?;
    let label = tool_event_label_tense("todo", Some(&view.arguments), ToolVerbTense::Perfect);
    render_todo_output(&label, &outcome.output, outcome.ok, mode)
}

/// 从 todo 工具的原始结果渲染清单。
///
/// 不依赖 TUI 的工具视图模型，CLI 的流式输出路径可以直接调用。
/// 标签由调用方给出：流式路径只缓存了标签，拿不到工具参数原文。
///
/// 参数:
/// - `label`: 已生成的工具事件标签
/// - `result_json`: todo 工具结果 JSON
/// - `ok`: 工具是否执行成功
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 结果可解析时返回清单文本
pub(crate) fn render_todo_output(
    label: &str,
    result_json: &str,
    ok: bool,
    mode: ToolCallDisplayMode,
) -> Option<String> {
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    let result = serde_json::from_str::<TodoResultView>(result_json).ok()?;
    let display_label = changed_item_label(label, &result.changed);
    let mut output = tool_event_text(&display_label, if ok { "ok" } else { "err" });

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

    // 摘要行走统一 gutter：x/x 计数（与沉底面板同一视觉语言，不画进度条）
    let mut stats = format!("{completed}/{total}");
    if in_progress > 0 {
        stats.push_str(&format!(" · {in_progress} active"));
    }
    if pending > 0 {
        stats.push_str(&format!(" · {pending} pending"));
    }
    if cancelled > 0 {
        stats.push_str(&format!(" · {cancelled} cancelled"));
    }
    output.push_str(&format!("\n\x1b[2m  └ {stats}\x1b[0m"));

    // Summary 模式：清单已由沉底面板常驻展示，历史区只保留摘要与当前进行中项，
    // 避免每次 todo 调用都在历史里重复整份清单
    if mode == ToolCallDisplayMode::Summary {
        if let Some(active) = result
            .items
            .iter()
            .find(|item| item.status == "in_progress")
        {
            output.push_str(&format!(
                "\n    {} {}",
                status_marker(&active.status),
                colorize_item(&active.status, &active.text)
            ));
        }
        return Some(output);
    }

    // Full 模式：扁平列表，进行中置顶；条目自带状态符，缩进对齐 gutter 续行列
    let mut items = result.items;
    items.sort_by_key(|item| status_rank(&item.status));

    for item in &items {
        let marker = status_marker(&item.status);
        let colored = colorize_item(&item.status, &item.text);
        output.push_str(&format!("\n    {marker} {colored}"));
    }
    Some(output)
}

/// 使用工具返回的变更条目替换内部 ID。
///
/// 参数:
/// - `label`: 原始工具标签（`Updating/Updated 动作 对象`）
/// - `changed`: 本次修改的条目
///
/// 返回:
/// - 包含条目状态和文本的可读标签
fn changed_item_label(label: &str, changed: &[TodoItemView]) -> String {
    // 标签动词由 tool_verb 生成（Updating/Updated），旧版 "Todo " 前缀已废弃
    let Some((verb, rest)) = label.split_once(' ') else {
        return label.to_string();
    };
    if !matches!(verb, "Updating" | "Updated") {
        return label.to_string();
    }
    let Some(action) = rest
        .split_whitespace()
        .next()
        .filter(|action| matches!(*action, "update" | "remove"))
    else {
        return label.to_string();
    };
    let Some(item) = changed.first() else {
        return label.to_string();
    };
    format!(
        "{verb} {action} {} {}",
        status_marker(&item.status),
        item.text
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::todo_style::status_marker;

    /// CLI 没有沉底面板，Full 渲染必须把每个条目都写出来，
    /// 否则计划只剩一行统计，用户无从查看。
    #[test]
    fn full_mode_lists_every_item_for_cli() {
        let result = r#"{"ok":true,"items":[
            {"text":"读取配置","status":"completed"},
            {"text":"改写解析","status":"in_progress"},
            {"text":"补测试","status":"pending"}
        ]}"#;

        let rendered = render_todo_output("Todo", result, true, ToolCallDisplayMode::Full).unwrap();

        assert!(rendered.contains("读取配置"));
        assert!(rendered.contains("改写解析"));
        assert!(rendered.contains("补测试"));
        assert!(rendered.contains("1/3"));
        assert!(!rendered.contains('█') && !rendered.contains('░'));
        assert!(rendered.contains('▶'));
    }

    /// 更新结果使用条目内容和状态，不暴露内部 ID（标签动词为 Updated/Updating）。
    #[test]
    fn update_summary_replaces_internal_id_with_changed_item() {
        let result = r#"{"ok":true,"changed":[
            {"id":"todo_1786108008960_2_34858","text":"补齐回归测试","status":"completed"}
        ],"items":[
            {"id":"todo_1786108008960_2_34858","text":"补齐回归测试","status":"completed"}
        ]}"#;

        let rendered = render_todo_output(
            "Updated update todo_1786108008960_2_34858",
            result,
            true,
            ToolCallDisplayMode::Summary,
        )
        .unwrap();

        assert!(rendered.contains("补齐回归测试"));
        assert!(rendered.contains('✓'));
        assert!(!rendered.contains("todo_1786108008960_2_34858"));
    }

    /// Summary 模式供 TUI 使用：清单由沉底面板常驻，历史区只留进行中项。
    #[test]
    fn summary_mode_keeps_only_the_active_item() {
        let result = r#"{"ok":true,"items":[
            {"text":"读取配置","status":"completed"},
            {"text":"改写解析","status":"in_progress"},
            {"text":"补测试","status":"pending"}
        ]}"#;

        let rendered =
            render_todo_output("Todo", result, true, ToolCallDisplayMode::Summary).unwrap();

        assert!(rendered.contains("改写解析"));
        assert!(!rendered.contains("补测试"));
    }

    /// 结果不是合法清单 JSON 时交回默认渲染，不吞掉输出。
    #[test]
    fn unparsable_result_falls_back() {
        assert!(render_todo_output("Todo", "not json", true, ToolCallDisplayMode::Full).is_none());
    }

    #[test]
    fn status_marker_covers_common_states() {
        assert!(status_marker("completed").contains('✓'));
        assert!(status_marker("in_progress").contains('▶'));
        assert!(status_marker("pending").contains('○'));
    }
}

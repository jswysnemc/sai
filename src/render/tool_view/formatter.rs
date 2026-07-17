use super::model::{PermissionAuditView, ToolView};
use crate::render::command_output::{
    render_command_block_with_action, render_command_result_view,
};
use crate::render::tool_event_line::{tool_event_label, tool_event_text};
use crate::render::ToolCallDisplayMode;
use serde_json::Value;

const PAYLOAD_LIMIT: usize = 2_400;

/// 渲染完整工具生命周期视图。
///
/// 参数:
/// - `view`: 工具生命周期数据
/// - `mode`: 工具展示模式
///
/// 返回:
/// - ANSI 工具视图文本
pub(crate) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> String {
    if mode == ToolCallDisplayMode::Hidden {
        return String::new();
    }

    // 命令类工具：始终用代码块展示命令；完成后追加 stdout/stderr 块
    if matches!(view.name.as_str(), "run_command" | "background_command") {
        return render_command_tool(view, mode);
    }
    if view.name == "todo" {
        if let Some(rendered) = super::todo::render(view, mode) {
            return rendered;
        }
    }

    let label = tool_event_label(&view.name, Some(&view.arguments));
    let status = match &view.outcome {
        Some(outcome) if outcome.ok => "ok",
        Some(_) => "err",
        None if view.arguments.trim().is_empty() => "arg",
        None => "run",
    };
    if mode == ToolCallDisplayMode::Summary {
        let mut output = tool_event_text(&label, status);
        if let Some(progress) = visible_progress(view.progress.as_deref()) {
            output.push_str(&format!("\n\x1b[2m  └─ {progress}\x1b[0m"));
        }
        // 失败时在 summary 也展示输出摘要，成功则保留状态行（不再整段吞掉）
        if let Some(outcome) = &view.outcome {
            if !outcome.ok && !outcome.output.trim().is_empty() {
                output.push('\n');
                output.push_str(&render_payload("output", &outcome.output));
            }
        }
        output.push_str(&render_permission(view.permission.as_ref()));
        return output;
    }

    let mut output = tool_event_text(&label, status);
    if !view.arguments.trim().is_empty() {
        output.push('\n');
        output.push_str(&render_payload("args", &view.arguments));
    }
    if let Some(progress) = visible_progress(view.progress.as_deref()) {
        output.push_str(&format!("\n\x1b[2m  ├─ {progress}\x1b[0m"));
    }
    if let Some(outcome) = &view.outcome {
        if !outcome.output.trim().is_empty() {
            output.push('\n');
            output.push_str(&render_payload("output", &outcome.output));
        }
    }
    output.push_str(&render_permission(view.permission.as_ref()));
    output
}

/// 渲染 run_command / background_command 的完整视图。
///
/// 参数:
/// - `view`: 工具生命周期
/// - `mode`: 展示模式
///
/// 返回:
/// - 命令代码块 + 可选结果块
fn render_command_tool(view: &ToolView, mode: ToolCallDisplayMode) -> String {
    let action = if view.name == "background_command" {
        "Background"
    } else {
        "Run"
    };
    let mut output = render_command_block_with_action(&view.arguments, action)
        .trim_end()
        .to_string();
    if let Some(progress) = visible_progress(view.progress.as_deref()) {
        output.push_str(&format!("\n\x1b[2m  ├─ {progress}\x1b[0m"));
    }
    if let Some(outcome) = &view.outcome {
        // Full / Summary 都展示结果块，保证历史长度随输出增长
        let results = render_command_result_view(&outcome.output);
        if !results.trim().is_empty() {
            output.push('\n');
            output.push_str(&results);
        } else {
            let status = if outcome.ok { "ok" } else { "err" };
            output.push_str(&format!(
                "\n{}",
                tool_event_text(&tool_event_label(&view.name, Some(&view.arguments)), status)
            ));
        }
        let _ = mode;
    }
    output.push_str(&render_permission(view.permission.as_ref()));
    output
}

/// 渲染附着在工具生命周期中的权限审计状态。
///
/// 参数:
/// - `permission`: 可选权限审计状态
///
/// 返回:
/// - 不重复工具内容的权限交互文本
fn render_permission(permission: Option<&PermissionAuditView>) -> String {
    let Some(permission) = permission else {
        return String::new();
    };
    match &permission.decision {
        Some(decision) => format!(
            "\n{}",
            crate::render::render_permission_decision(decision)
        ),
        None => format!(
            "\n{}",
            crate::render::render_permission_controls(
                permission.selected,
                permission.reply_draft.as_deref(),
            )
        ),
    }
}

/// 渲染单次工具调用，供普通 CLI 使用。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments`: 工具参数
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 工具调用文本
pub(crate) fn render_call(name: &str, arguments: &str, mode: ToolCallDisplayMode) -> String {
    render(
        &ToolView::running(name.to_string(), arguments.to_string()),
        mode,
    )
}

/// 渲染单次工具结果，供普通 CLI 使用。
///
/// 参数:
/// - `name`: 工具名称
/// - `ok`: 工具是否成功
/// - `output`: 工具输出
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 工具结果文本
pub(crate) fn render_result(
    name: &str,
    ok: bool,
    output: &str,
    mode: ToolCallDisplayMode,
) -> String {
    let mut view = ToolView::running(name.to_string(), String::new());
    view.finish(ok, output.to_string());
    render(&view, mode)
}

/// 渲染层级工具载荷。
///
/// 参数:
/// - `label`: 载荷名称
/// - `payload`: 原始 JSON 或文本
///
/// 返回:
/// - Codex 风格的层级载荷块
fn render_payload(label: &str, payload: &str) -> String {
    let formatted = serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| payload.trim().to_string());
    let truncated = truncate_chars(&formatted, PAYLOAD_LIMIT);
    let mut lines = truncated.lines();
    let first = lines.next().unwrap_or_default();
    let mut output = format!("\x1b[2m  └─ {label}: {first}\x1b[0m");
    for line in lines {
        output.push_str(&format!("\n\x1b[2m     {line}\x1b[0m"));
    }
    output
}

/// 过滤内部进度消息与空进度。
///
/// 参数:
/// - `progress`: 可选进度信息
///
/// 返回:
/// - 可展示的进度文本
fn visible_progress(progress: Option<&str>) -> Option<&str> {
    progress
        .map(str::trim)
        .filter(|message| !message.is_empty() && !message.starts_with("__"))
}

/// 按字符数量截断工具载荷。
///
/// 参数:
/// - `text`: 原始文本
/// - `limit`: 最大字符数
///
/// 返回:
/// - 截断后的文本
fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut output = text.chars().take(limit).collect::<String>();
    output.push_str("\n...");
    output
}

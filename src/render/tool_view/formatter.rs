use super::model::{PermissionAuditView, ToolView};
use crate::permission::PermissionDecision;
use crate::render::command_output::render_command_block_with_action;
use crate::render::command_result_block::{
    render_command_result_view_with_limit, render_completed_command_output,
    render_live_command_output,
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

    // 只有前台命令使用命令输出专用视图，后台命令按普通工具载荷展示
    if view.name == "run_command" {
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
    let denied = permission_denied(view.permission.as_ref());
    if mode == ToolCallDisplayMode::Summary {
        let mut output = tool_event_text(&label, status);
        if let Some(progress) = visible_progress(view.progress.as_deref()) {
            output.push_str(&format!("\n\x1b[2m  └─ {progress}\x1b[0m"));
        }
        // 失败时在 summary 也展示输出摘要，成功则保留状态行（不再整段吞掉）
        if let Some(outcome) = &view.outcome {
            if !outcome.ok && !denied && !outcome.output.trim().is_empty() {
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
        if !denied && !outcome.output.trim().is_empty() {
            output.push('\n');
            output.push_str(&render_payload("output", &outcome.output));
        }
    }
    output.push_str(&render_permission(view.permission.as_ref()));
    output
}

/// 渲染前台 run_command 的完整视图。
///
/// 参数:
/// - `view`: 工具生命周期
/// - `mode`: 展示模式
///
/// 返回:
/// - 命令代码块 + 可选结果块
fn render_command_tool(view: &ToolView, mode: ToolCallDisplayMode) -> String {
    let action = "Run";
    let mut output = render_command_block_with_action(&view.arguments, action)
        .trim_end()
        .to_string();
    if let Some(progress) = visible_progress(view.progress.as_deref()) {
        output.push_str(&format!("\n\x1b[2m  ├─ {progress}\x1b[0m"));
    }
    // 权限拒绝的失败输出与「已拒绝」决定行重复，跳过结果块
    let denied = permission_denied(view.permission.as_ref());
    if let Some(outcome) = view.outcome.as_ref().filter(|_| !denied) {
        let results = if view.name == "run_command" {
            let stdout = view.command_stdout_text();
            let stderr = view.command_stderr_text();
            render_completed_command_output(
                &outcome.output,
                &stdout,
                &stderr,
                view.command_expanded,
            )
        } else {
            render_command_result_view_with_limit(&outcome.output, None)
        };
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
    } else if view.name == "run_command" {
        let stdout = view.command_stdout_text();
        let stderr = view.command_stderr_text();
        let preview = render_live_command_output(&stdout, &stderr, view.command_expanded);
        if !preview.trim().is_empty() {
            output.push('\n');
            output.push_str(&preview);
        }
    }
    output.push_str(&render_permission(view.permission.as_ref()));
    output
}

/// 判断权限审计是否以拒绝告终。
///
/// 参数:
/// - `permission`: 可选权限审计状态
///
/// 返回:
/// - 拒绝时返回 true
fn permission_denied(permission: Option<&PermissionAuditView>) -> bool {
    permission
        .and_then(|item| item.decision.as_ref())
        .is_some_and(|decision| matches!(decision, PermissionDecision::Deny { .. }))
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
        Some(decision) => format!("\n{}", crate::render::render_permission_decision(decision)),
        None => {
            let status = crate::render::render_auto_audit_status(permission.auto_audit);
            let controls = crate::render::render_permission_controls(
                permission.selected,
                permission.reply_draft.as_deref(),
            );
            if status.is_empty() {
                format!("\n{controls}")
            } else {
                format!("\n{status}\n{controls}")
            }
        }
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
    use crate::render::fold_text::{
        fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES,
    };
    let formatted = serde_json::from_str::<Value>(payload)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| payload.trim().to_string());
    // 1. 先按字符上限粗截，再按显示行首尾折叠（后台/子智能体长结果）
    let truncated = truncate_chars(&formatted, PAYLOAD_LIMIT);
    let wrapped = wrap_display_lines(&truncated, terminal_wrap_width().saturating_sub(8).max(8));
    let (visible, omitted) = fold_display_lines(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, false);
    let mut output = String::new();
    for (index, line) in visible.iter().enumerate() {
        let text = if line == "__OMITTED__" {
            format!("… +{omitted} lines")
        } else {
            line.clone()
        };
        if index == 0 {
            output.push_str(&format!("\x1b[2m  └─ {label}: {text}\x1b[0m"));
        } else {
            output.push_str(&format!("\n\x1b[2m     {text}\x1b[0m"));
        }
    }
    if output.is_empty() {
        output.push_str(&format!("\x1b[2m  └─ {label}:\x1b[0m"));
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

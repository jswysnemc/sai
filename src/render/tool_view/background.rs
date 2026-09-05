use super::model::ToolView;
use crate::render::command_output::render_command_block_with_action;
use crate::render::command_result_block::render_live_command_output;
use crate::render::status_style::ToolHealth;
use crate::render::terminal_text as t;
use serde_json::Value;

impl ToolView {
    /// 返回可持续更新的后台启动任务标识，无参数，不含历史 output/wait 快照。
    pub(crate) fn background_task_id(&self) -> Option<String> {
        let value = result_value(self)?;
        let promoted = self.name == "run_command" && value["mode"] == "background";
        let started = self.name == "background_command"
            && serde_json::from_str::<Value>(&self.arguments).ok()?["action"] == "start";
        if !promoted && !started {
            return None;
        }
        value
            .pointer("/task/id")
            .or_else(|| value.get("task_id"))?
            .as_str()
            .map(str::to_string)
    }

    /// 判断启动卡片对应的后台进程是否仍运行，无参数，返回是否需要动效。
    pub(crate) fn is_running_background(&self) -> bool {
        self.background_task_id().is_some()
            && result_value(self).is_some_and(|value| {
                value
                    .pointer("/task/status")
                    .and_then(Value::as_str)
                    .unwrap_or("running")
                    == "running"
            })
    }
}

/// 解析已完成调用的 JSON；参数为工具卡，返回可选结构化结果。
fn result_value(view: &ToolView) -> Option<Value> {
    serde_json::from_str(&view.outcome.as_ref()?.output).ok()
}

/// 提取后台卡片的命令；参数为工具卡，返回结果中的命令或启动参数。
pub(crate) fn background_task_command(view: &ToolView) -> String {
    result_value(view)
        .and_then(|value| value.pointer("/task/command")?.as_str().map(str::to_string))
        .or_else(|| {
            crate::render::tool_event_line::lenient_string_field(&view.arguments, "command")
        })
        .unwrap_or_default()
}

/// 合并实时流与后台结果；参数为工具卡和结果，返回两个输出流。
fn streams(view: &ToolView, value: &Value) -> (String, String) {
    let stream = |name: &str, partial: &str, captured: std::borrow::Cow<'_, str>| {
        if value["live_output"] != true && !captured.is_empty() {
            captured.into_owned()
        } else {
            value
                .get(name)
                .or_else(|| value.get(partial))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        }
    };
    (
        stream("stdout", "partial_stdout", view.command_stdout_text()),
        stream("stderr", "partial_stderr", view.command_stderr_text()),
    )
}

/// 映射后台进程状态；参数为状态键，返回自然文案及颜色语义。
fn state_label(status: &str) -> (&'static str, ToolHealth) {
    match status {
        "running" => (
            t("Running in background", "后台运行中"),
            ToolHealth::Pending,
        ),
        "exited" | "completed" => (t("Finished", "已结束"), ToolHealth::Ok),
        "stopped" | "cancelled" => (t("Stopped", "已停止"), ToolHealth::Pending),
        "timed_out" => (t("Timed out", "运行超时"), ToolHealth::Err),
        "failed" => (t("Failed", "执行失败"), ToolHealth::Err),
        "untracked" => (t("No longer tracked", "已停止跟踪"), ToolHealth::Pending),
        _ => (t("Background command", "后台命令"), ToolHealth::Pending),
    }
}

/// 【终端】【后台日志】按命令卡片渲染后台启动、输出、等待和停止结果。
///
/// 参数: `view` 为工具生命周期，`frame` 为当前动画帧
/// 返回: 后台命令专用视图；其他工具返回空
pub(super) fn render(view: &ToolView, frame: usize) -> Option<String> {
    let value = result_value(view);
    let promoted = view.name == "run_command"
        && value
            .as_ref()
            .is_some_and(|value| value["mode"] == "background");
    if view.name != "background_command" && !promoted {
        return None;
    }
    let Some(value) = value else {
        if view.outcome.is_none()
            && crate::render::tool_event_line::lenient_string_field(&view.arguments, "action")
                .as_deref()
                == Some("start")
        {
            let output = render_command_block_with_action(
                &serde_json::json!({"command": background_task_command(view)}).to_string(),
                t("Starting in background", "正在启动后台命令"),
                ToolHealth::Pending,
            );
            return Some(crate::render::content_indent::animate_guide_marker(
                output.trim_end(),
                frame,
            ));
        }
        return None;
    };
    let Some(task) = value.get("task").filter(|task| task.is_object()) else {
        if let Some(tasks) = value.get("tasks").and_then(Value::as_array) {
            let mut output = crate::render::tool_event_line::tool_event_text(
                &format!("{} · {}", t("Background commands", "后台命令"), tasks.len()),
                "ok",
            );
            for task in tasks
                .iter()
                .take(if crate::render::render_expand::expand_override() {
                    usize::MAX
                } else {
                    5
                })
            {
                output.push_str(&format!(
                    "\n  {} · {}\n    $ {}",
                    task["id"].as_str().unwrap_or_default(),
                    state_label(task["status"].as_str().unwrap_or_default()).0,
                    task["command"].as_str().unwrap_or_default()
                ));
            }
            if tasks.len() > 5 && !crate::render::render_expand::expand_override() {
                output.push_str(&format!(
                    "\n  {}",
                    crate::render::omitted_line::render_fold_hint(
                        &format!("{} {}", tasks.len() - 5, t("tasks hidden", "项任务已折叠")),
                        Some("Ctrl+O")
                    )
                ));
            }
            return Some(output);
        }
        return None;
    };
    let status = task["status"].as_str().unwrap_or("running");
    let (label, health) = state_label(status);
    let command = background_task_command(view);
    let mut output = render_command_block_with_action(
        &serde_json::json!({"command": command}).to_string(),
        label,
        health,
    )
    .trim_end()
    .to_string();
    if status == "running" && frame > 0 && view.is_running_background() {
        output = crate::render::content_indent::animate_guide_marker(&output, frame);
    }
    let id = task["id"].as_str().unwrap_or_default();
    output.push_str(&format!("\n\x1b[2m  {id} · /ps\x1b[0m"));
    let (stdout, stderr) = streams(view, &value);
    let body = render_live_command_output(&stdout, &stderr, view.command_expanded);
    if !body.trim().is_empty() {
        output.push('\n');
        output.push_str(body.trim_end());
    } else if status == "running" {
        output.push_str(&format!(
            "\n\x1b[2m  {}\x1b[0m",
            t("Waiting for output", "等待输出")
        ));
    }
    if value["needs_attention"] == true {
        output.push_str(&format!(
            "\n\x1b[33m  {}\x1b[0m",
            t(
                "Still running · check recent output",
                "仍在运行，请检查近期输出"
            )
        ));
    }
    if value["stdout_truncated"] == true || value["stderr_truncated"] == true {
        output.push_str(&format!(
            "\n\x1b[2m  {}\x1b[0m",
            t(
                "Recent log excerpt · /ps for more",
                "当前为日志片段，可在 /ps 查看更多"
            )
        ));
    }
    output.push_str(&super::formatter::render_permission(
        view.permission.as_ref(),
    ));
    Some(output)
}

/// 【终端】【后台全文】收集后台命令的已读取日志，避免分页器展示协议 JSON。
///
/// 参数: `view` 为工具卡
/// 返回: 可读状态和完整已读取输出，其他工具返回空
pub(crate) fn background_pager_body(view: &ToolView) -> Option<String> {
    let value = result_value(view)?;
    if view.name != "background_command" && value["mode"] != "background" {
        return None;
    }
    let (stdout, stderr) = streams(view, &value);
    let mut parts = vec![];
    if !stdout.is_empty() {
        parts.push(format!("── stdout ──\n{stdout}"));
    }
    if !stderr.is_empty() {
        parts.push(format!("── stderr ──\n{stderr}"));
    }
    if parts.is_empty() {
        return crate::render::render_expand::with_expanded_render(|| render(view, 0));
    }
    Some(parts.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    /// 启动返回之前也保留命令行及活动状态，避免中途切换成原始参数展示。
    #[test]
    fn background_start_shows_command_while_pending() {
        let view = ToolView::running(
            "background_command".into(),
            r#"{"action":"start","command":"cargo check"}"#.into(),
        );
        let rendered = strip_ansi_for_test(&render(&view, 1).unwrap());
        assert!(rendered.contains("$ cargo check"));
        assert!(!rendered.contains("\"action\""));
    }

    /// 后台日志共享前台折叠规则，保留命令符并在全文视图恢复所有已读取行。
    #[test]
    fn background_logs_fold_and_expand_as_command_output() {
        let mut view =
            ToolView::running("background_command".into(), r#"{"action":"output"}"#.into());
        let log = (1..=30)
            .map(|n| format!("log-{n}"))
            .collect::<Vec<_>>()
            .join("\n");
        view.finish(true, serde_json::json!({"ok":true,"task":{"id":"task","command":"cargo check","status":"running"},"stdout":log}).to_string());
        let rendered = strip_ansi_for_test(&render(&view, 0).unwrap());
        assert!(rendered.contains("$ cargo check"));
        assert!(rendered.contains("Ctrl+O"));
        assert!(!rendered.contains("log-15"));
        assert!(background_pager_body(&view).unwrap().contains("log-15"));
        assert!(!rendered.contains("\"stdout\""));
    }

    /// 转后台时已经收集的标准输出和错误输出都保留。
    #[test]
    fn promotion_preserves_both_output_streams() {
        let mut view = ToolView::running("run_command".into(), r#"{"command":"build"}"#.into());
        view.finish(true, serde_json::json!({"mode":"background","task_id":"task","task":{"id":"task","status":"running"},"partial_stdout":"working","partial_stderr":"diagnostic"}).to_string());
        let rendered = strip_ansi_for_test(&render(&view, 1).unwrap());
        assert!(rendered.contains("$ build"));
        assert!(rendered.contains("working") && rendered.contains("diagnostic"));
        assert!(view.is_running_background());
    }
}

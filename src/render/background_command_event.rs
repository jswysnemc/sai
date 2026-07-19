use crate::render::terminal_text as t;
use serde_json::Value;

/// 生成后台命令调用展示标签。
///
/// 参数:
/// - `arguments`: `background_command` 工具参数
///
/// 返回:
/// - 面向终端展示的后台命令动作标签
pub(crate) fn background_command_call_label(arguments: Option<&str>) -> String {
    let Some(arguments) = arguments else {
        return t("Background command", "后台命令").to_string();
    };
    let action = background_action(arguments).unwrap_or_else(|| "command".to_string());
    match action.as_str() {
        "start" => label_with_target(
            t("Background start", "启动后台命令"),
            start_target(arguments),
        ),
        "list" => t("Background list", "后台命令列表").to_string(),
        "output" => label_with_target(
            t("Background output", "后台命令输出"),
            task_id_target(arguments),
        ),
        "stop" => label_with_target(
            t("Background stop", "停止后台命令"),
            task_id_target(arguments),
        ),
        "cleanup" => t("Background cleanup", "清理后台命令").to_string(),
        _ => t("Background command", "后台命令").to_string(),
    }
}

/// 判断后台命令工具调用是否应渲染为命令块。
///
/// 参数:
/// - `arguments`: `background_command` 工具参数
///
/// 返回:
/// - 是否为启动后台命令
pub(crate) fn is_background_command_start(arguments: &str) -> bool {
    background_action(arguments)
        .map(|action| action == "start")
        .unwrap_or(false)
}

/// 返回后台命令启动时的命令块动作标题。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 命令块动作标题
pub(crate) fn background_command_block_action() -> &'static str {
    t("Background", "后台")
}

/// 生成后台命令结果摘要。
///
/// 参数:
/// - `output`: `background_command` 工具输出
///
/// 返回:
/// - 面向终端展示的结果摘要
pub(crate) fn background_command_result_label(output: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    if let Some(task) = value.get("task") {
        if value.get("was_running").is_some() {
            return Some(stop_result_label(task, &value));
        }
        if value.get("stdout").is_some() || value.get("stderr").is_some() {
            return Some(output_result_label(task, &value));
        }
        return Some(start_result_label(task));
    }
    if let Some(tasks) = value.get("tasks").and_then(Value::as_array) {
        return Some(list_result_label(tasks));
    }
    if value.get("removed").is_some() || value.get("remaining").is_some() {
        return Some(cleanup_result_label(&value));
    }
    None
}

/// 读取后台命令动作。
///
/// 参数:
/// - `arguments`: 工具参数文本
///
/// 返回:
/// - 后台命令动作
fn background_action(arguments: &str) -> Option<String> {
    serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| string_field(&value, "action"))
        .or_else(|| json_string_field_from_partial(arguments, "action"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 读取启动动作展示对象。
///
/// 参数:
/// - `arguments`: 工具参数文本
///
/// 返回:
/// - 标签或命令摘要
fn start_target(arguments: &str) -> Option<String> {
    serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| string_field(&value, "label").or_else(|| string_field(&value, "command")))
        .or_else(|| {
            json_string_field_from_partial(arguments, "label")
                .or_else(|| json_string_field_from_partial(arguments, "command"))
        })
        .map(compact_text)
}

/// 读取任务 ID 展示对象。
///
/// 参数:
/// - `arguments`: 工具参数文本
///
/// 返回:
/// - 任务 ID 摘要
fn task_id_target(arguments: &str) -> Option<String> {
    serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| string_field(&value, "task_id"))
        .or_else(|| json_string_field_from_partial(arguments, "task_id"))
        .map(short_id)
}

/// 组装带目标的标签。
///
/// 参数:
/// - `prefix`: 标签前缀
/// - `target`: 可选展示对象
///
/// 返回:
/// - 展示标签
fn label_with_target(prefix: &str, target: Option<String>) -> String {
    target
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("{prefix} {value}"))
        .unwrap_or_else(|| prefix.to_string())
}

/// 生成启动结果摘要。
///
/// 参数:
/// - `task`: 后台任务 JSON
///
/// 返回:
/// - 启动结果摘要
fn start_result_label(task: &Value) -> String {
    let label = string_field(task, "label").unwrap_or_else(|| "task".to_string());
    let id = string_field(task, "id").map(short_id);
    let pid = task.get("pid").and_then(Value::as_u64);
    let timeout = task.get("timeout_seconds").and_then(Value::as_u64);
    let mut parts = vec![format!(
        "{} {}",
        t("Background started", "后台命令已启动"),
        compact_text(label)
    )];
    if let Some(id) = id {
        parts.push(format!("id={id}"));
    }
    if let Some(pid) = pid {
        parts.push(format!("pid={pid}"));
    }
    if let Some(timeout) = timeout {
        parts.push(format!("timeout={}", timeout_label(timeout)));
    }
    parts.join(" ")
}

/// 生成列表结果摘要。
///
/// 参数:
/// - `tasks`: 后台任务列表
///
/// 返回:
/// - 列表结果摘要
fn list_result_label(tasks: &[Value]) -> String {
    let running = count_status(tasks, "running");
    let exited = count_status(tasks, "exited");
    let stopped = count_status(tasks, "stopped");
    let timed_out = count_status(tasks, "timed_out");
    format!(
        "{} {}={running} {}={exited} {}={stopped} {}={timed_out}",
        t("Background list", "后台命令列表"),
        t("running", "运行中"),
        t("exited", "已退出"),
        t("stopped", "已停止"),
        t("timed_out", "已超时")
    )
}

/// 生成输出读取结果摘要。
///
/// 参数:
/// - `task`: 后台任务 JSON
/// - `value`: 完整输出 JSON
///
/// 返回:
/// - 输出读取结果摘要
fn output_result_label(task: &Value, value: &Value) -> String {
    let id = string_field(task, "id")
        .map(short_id)
        .unwrap_or_else(|| "task".to_string());
    let stdout_lines = text_line_count(value.get("stdout"));
    let stderr_lines = text_line_count(value.get("stderr"));
    format!(
        "{} {id} stdout={stdout_lines} {} stderr={stderr_lines} {}",
        t("Background output", "后台命令输出"),
        t("lines", "行"),
        t("lines", "行")
    )
}

/// 生成停止结果摘要。
///
/// 参数:
/// - `task`: 后台任务 JSON
/// - `value`: 完整停止结果 JSON
///
/// 返回:
/// - 停止结果摘要
fn stop_result_label(task: &Value, value: &Value) -> String {
    let id = string_field(task, "id")
        .map(short_id)
        .unwrap_or_else(|| "task".to_string());
    let status = string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    let was_running = value
        .get("was_running")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if was_running {
        format!(
            "{} {id} {}",
            t("Background stop", "停止后台命令"),
            localized_status(&status)
        )
    } else {
        format!(
            "{} {id} {}_{}",
            t("Background stop", "停止后台命令"),
            t("already", "已经"),
            localized_status(&status)
        )
    }
}

/// 生成清理结果摘要。
///
/// 参数:
/// - `value`: 清理结果 JSON
///
/// 返回:
/// - 清理结果摘要
fn cleanup_result_label(value: &Value) -> String {
    let removed = value
        .get("removed")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let remaining = value
        .get("remaining")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    format!(
        "{} {}={removed} {}={remaining}",
        t("Background cleanup", "清理后台命令"),
        t("removed", "已移除"),
        t("remaining", "剩余")
    )
}

/// 返回后台任务状态的本地化名称。
///
/// 参数:
/// - `status`: 后台任务状态
///
/// 返回:
/// - 本地化状态名称
fn localized_status(status: &str) -> &str {
    match status {
        "running" => t("running", "运行中"),
        "exited" => t("exited", "已退出"),
        "stopped" => t("stopped", "已停止"),
        "timed_out" => t("timed_out", "已超时"),
        _ => status,
    }
}

/// 统计指定状态任务数量。
///
/// 参数:
/// - `tasks`: 任务列表
/// - `status`: 目标状态
///
/// 返回:
/// - 目标状态数量
fn count_status(tasks: &[Value], status: &str) -> usize {
    tasks
        .iter()
        .filter(|task| {
            task.get("status")
                .and_then(Value::as_str)
                .map(|value| value == status)
                .unwrap_or(false)
        })
        .count()
}

/// 统计可选文本行数。
///
/// 参数:
/// - `value`: 可选文本 JSON
///
/// 返回:
/// - 文本行数
fn text_line_count(value: Option<&Value>) -> usize {
    value
        .and_then(Value::as_str)
        .map(|text| {
            if text.is_empty() {
                0
            } else {
                text.lines().count()
            }
        })
        .unwrap_or_default()
}

/// 读取字符串字段。
///
/// 参数:
/// - `value`: JSON 对象
/// - `key`: 字段名
///
/// 返回:
/// - 非空字符串字段
fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// 压缩展示文本。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 单行展示文本
fn compact_text(value: String) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() <= 48 {
        value
    } else {
        format!("{}...", value.chars().take(45).collect::<String>())
    }
}

/// 缩短任务 ID。
///
/// 参数:
/// - `value`: 原始 ID
///
/// 返回:
/// - 适合单行展示的 ID
fn short_id(value: String) -> String {
    if value.chars().count() <= 18 {
        value
    } else {
        format!("{}...", value.chars().take(15).collect::<String>())
    }
}

/// 生成超时展示文本。
///
/// 参数:
/// - `seconds`: 超时秒数
///
/// 返回:
/// - 超时展示文本
fn timeout_label(seconds: u64) -> String {
    if seconds == 0 {
        "none".to_string()
    } else {
        format!("{seconds}s")
    }
}

/// 从 JSON 片段中读取指定字符串字段。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 字段值
fn json_string_field_from_partial(raw: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let marker_start = raw.find(&marker)?;
    let after_marker = &raw[marker_start + marker.len()..];
    let colon = after_marker.find(':')?;
    let after_colon = after_marker[colon + 1..].trim_start();
    let rest = after_colon.strip_prefix('"')?;
    let mut escaped = false;
    let mut output = String::new();
    for ch in rest.chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(output);
        }
        output.push(ch);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn call_label_describes_background_actions() {
        assert_eq!(
            background_command_call_label(Some(
                r#"{"action":"start","label":"dev server","command":"npm run dev"}"#
            )),
            format!("{} dev server", t("Background start", "启动后台命令"))
        );
        assert_eq!(
            background_command_call_label(Some(r#"{"action":"list"}"#)),
            t("Background list", "后台命令列表")
        );
        assert_eq!(
            background_command_call_label(Some(
                r#"{"action":"output","task_id":"1730000000-12345"}"#
            )),
            format!(
                "{} 1730000000-12345",
                t("Background output", "后台命令输出")
            )
        );
        assert_eq!(
            background_command_call_label(Some(r#"{"action":"cleanup"}"#)),
            t("Background cleanup", "清理后台命令")
        );
    }

    #[test]
    fn start_result_summarizes_task_identity() {
        let output = json!({
            "ok": true,
            "task": {
                "id": "1730000000-12345",
                "label": "dev server",
                "pid": 12345,
                "timeout_seconds": 0
            }
        })
        .to_string();

        assert_eq!(
            background_command_result_label(&output).unwrap(),
            format!(
                "{} dev server id=1730000000-12345 pid=12345 timeout=none",
                t("Background started", "后台命令已启动")
            )
        );
    }

    #[test]
    fn list_output_stop_and_cleanup_results_are_compact() {
        let list = json!({
            "ok": true,
            "tasks": [
                {"status": "running"},
                {"status": "running"},
                {"status": "exited"},
                {"status": "stopped"},
                {"status": "timed_out"}
            ]
        })
        .to_string();
        let output = json!({
            "ok": true,
            "task": {"id": "1730000000-12345"},
            "stdout": "one\ntwo",
            "stderr": ""
        })
        .to_string();
        let stop = json!({
            "ok": true,
            "was_running": true,
            "task": {"id": "1730000000-12345", "status": "stopped"}
        })
        .to_string();
        let cleanup = json!({
            "ok": true,
            "removed": ["a", "b"],
            "remaining": 1
        })
        .to_string();

        assert_eq!(
            background_command_result_label(&list).unwrap(),
            format!(
                "{} {}=2 {}=1 {}=1 {}=1",
                t("Background list", "后台命令列表"),
                t("running", "运行中"),
                t("exited", "已退出"),
                t("stopped", "已停止"),
                t("timed_out", "已超时")
            )
        );
        assert_eq!(
            background_command_result_label(&output).unwrap(),
            format!(
                "{} 1730000000-12345 stdout=2 {} stderr=0 {}",
                t("Background output", "后台命令输出"),
                t("lines", "行"),
                t("lines", "行")
            )
        );
        assert_eq!(
            background_command_result_label(&stop).unwrap(),
            format!(
                "{} 1730000000-12345 {}",
                t("Background stop", "停止后台命令"),
                t("stopped", "已停止")
            )
        );
        assert_eq!(
            background_command_result_label(&cleanup).unwrap(),
            format!(
                "{} {}=2 {}=1",
                t("Background cleanup", "清理后台命令"),
                t("removed", "已移除"),
                t("remaining", "剩余")
            )
        );
    }
}

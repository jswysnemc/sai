use crate::render::terminal_text as t;
use crate::render::tool_event_line::ToolVerbTense;
use serde_json::Value;

/// 生成后台命令调用展示标签。
///
/// 参数:
/// - `arguments`: `background_command` 工具参数
///
/// 返回:
/// - 面向终端展示的后台命令动作标签
#[cfg(test)]
fn background_command_call_label(arguments: Option<&str>) -> String {
    background_command_call_label_tense(arguments, ToolVerbTense::Progressive)
}

/// 按后台命令动作及生命周期生成自然语言标签。
///
/// 参数: `arguments` 为完整或部分工具参数，`tense` 区分进行中和完成事件
/// 返回: 展示动作与目标的标签
pub(crate) fn background_command_call_label_tense(
    arguments: Option<&str>,
    tense: ToolVerbTense,
) -> String {
    let Some(arguments) = arguments else {
        return t("Background command", "后台命令").to_string();
    };
    let done = tense == ToolVerbTense::Perfect;
    let action = background_action(arguments).unwrap_or_else(|| "command".to_string());
    match action.as_str() {
        "start" => label_with_target(
            if done {
                "Started background command"
            } else {
                "Starting background command"
            },
            start_target(arguments),
        ),
        "list" => if done {
            "Checked background commands"
        } else {
            "Checking background commands"
        }
        .to_string(),
        "output" => label_with_target(
            if done {
                "Read background output"
            } else {
                "Reading background output"
            },
            task_id_target(arguments),
        ),
        "wait" => label_with_target(
            if done {
                "Waited for background command"
            } else {
                "Waiting for background command"
            },
            task_id_target(arguments),
        ),
        "stop" => label_with_target(
            if done {
                "Stopped background command"
            } else {
                "Stopping background command"
            },
            task_id_target(arguments),
        ),
        "cleanup" => if done {
            "Cleared finished background commands"
        } else {
            "Clearing finished background commands"
        }
        .to_string(),
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
    t("Starting in background", "正在后台启动")
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
        if value.get("waited").is_some() {
            return Some(wait_result_label(task, &value));
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
    let mut parts = vec![format!("Started {} in background", compact_text(label))];
    if let Some(id) = id {
        parts.push(id);
    }
    if let Some(pid) = pid {
        parts.push(format!("PID {pid}"));
    }
    if let Some(timeout) = timeout.filter(|seconds| *seconds > 0) {
        parts.push(format!("timeout {}", timeout_label(timeout)));
    }
    parts.join(" · ")
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
        "Background commands · {running} running · {exited} finished · {stopped} stopped · {timed_out} timed out"
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
        "Read background output {id} · {stdout_lines} stdout lines · {stderr_lines} stderr lines"
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
        format!("Stopped background command {id}")
    } else {
        format!(
            "Background command {id} already {}",
            localized_status(&status)
        )
    }
}

/// 生成等待结果摘要。
///
/// 参数:
/// - `task`: 后台任务 JSON
/// - `value`: 完整等待结果 JSON
///
/// 返回:
/// - 等待结果摘要
fn wait_result_label(task: &Value, value: &Value) -> String {
    let id = string_field(task, "id")
        .map(short_id)
        .unwrap_or_else(|| "task".to_string());
    if value
        .get("timeout")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return format!(
            "Stopped waiting for background command {id} · still {}",
            localized_status(
                &string_field(task, "status").unwrap_or_else(|| "running".to_string())
            )
        );
    }
    let status = string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    format!("Background command {id} {}", localized_status(&status))
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
    format!("Cleared {removed} finished background commands · {remaining} remaining")
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
        "exited" => t("finished", "已结束"),
        "stopped" => t("stopped", "已停止"),
        "timed_out" => t("timed out", "已超时"),
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
    // 按显示列数截断：中文路径按字符数截断会撑到近两倍宽
    crate::render::clip_to_width(&value, 48, "...")
}

/// 缩短任务 ID。
///
/// 参数:
/// - `value`: 原始 ID
///
/// 返回:
/// - 适合单行展示的 ID
fn short_id(value: String) -> String {
    crate::render::clip_to_width(&value, 18, "...")
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
mod tests;

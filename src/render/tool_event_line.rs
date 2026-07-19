use crate::render::background_command_event::background_command_call_label;
use crate::render::status_style::color_status;
use crate::render::style::TOOL_BULLET;
use serde_json::Value;
use std::path::Path;

/// 生成工具调用展示标签。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 工具参数 JSON 文本
///
/// 返回:
/// - 面向终端展示的短标签
pub(crate) fn tool_event_label(name: &str, arguments: Option<&str>) -> String {
    if name == "background_command" {
        return background_command_call_label(arguments);
    }
    let action = tool_action(name);
    let suffix = arguments.and_then(|arguments| tool_suffix_from_text(name, arguments));
    match suffix {
        Some(suffix) if !suffix.trim().is_empty() => format!("{action} {suffix}"),
        _ if action == "Tool" => format!("Tool {name}"),
        _ => action.to_string(),
    }
}

/// 生成工具状态事件文本。
///
/// 参数:
/// - `label`: 工具展示标签
/// - `status`: 状态文本
///
/// 返回:
/// - 工具状态事件文本
pub(crate) fn tool_event_text(label: &str, status: &str) -> String {
    format!("{TOOL_BULLET} {label} {}", color_status(status))
}

/// 生成工具状态文本。
///
/// 参数:
/// - `label`: 工具展示标签
/// - `status`: 工具状态
///
/// 返回:
/// - 可直接写入终端的单行状态文本
pub(crate) fn tool_call_status_text(label: &str, status: &str) -> String {
    format!("{TOOL_BULLET} {label} {}", color_status(status))
}

/// 返回工具动作短名。
///
/// 参数:
/// - `name`: 工具原始名称
///
/// 返回:
/// - 动作短名
fn tool_action(name: &str) -> &'static str {
    match name {
        "run_command" => "Run",
        "edit_file" => "Edit",
        "read_file" => "Read",
        "trash_path" => "Trash",
        "glob" | "find_files" => "Find",
        "grep" | "search_text" => "Search",
        "subagent" => "Subagent",
        "todo" => "Todo",
        "cron" => "Schedule",
        "check_os_info" => "Check",
        "load" => "Load",
        "create_directory" => "Create",
        "list_directory" => "List",
        _ => "Tool",
    }
}

/// 解析工具参数 JSON。
///
/// 参数:
/// - `arguments`: 工具参数 JSON 文本
///
/// 返回:
/// - 解析后的 JSON 值
fn parse_arguments(arguments: &str) -> Option<Value> {
    serde_json::from_str::<Value>(arguments).ok()
}

/// 从完整或部分参数文本中提取展示对象。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 工具参数文本
///
/// 返回:
/// - 可展示对象文本
fn tool_suffix_from_text(name: &str, arguments: &str) -> Option<String> {
    parse_arguments(arguments)
        .and_then(|value| tool_suffix(name, &value))
        .or_else(|| tool_suffix_from_partial_text(name, arguments))
}

/// 提取工具展示对象。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 工具参数
///
/// 返回:
/// - 可展示对象文本
fn tool_suffix(name: &str, arguments: &Value) -> Option<String> {
    match name {
        "run_command" => string_field(arguments, &["command"]).map(command_summary),
        "edit_file" | "trash_path" => string_field(arguments, &["path"]).map(file_basename),
        "read_file" => read_file_suffix(arguments),
        "glob" | "find_files" | "grep" | "search_text" => {
            string_field(arguments, &["include", "pattern"]).map(compact_text)
        }
        "subagent" => subagent_suffix(arguments),
        "todo" | "cron" => action_suffix(arguments),
        "load" => load_suffix(arguments),
        _ => None,
    }
}

/// 从不完整 JSON 参数文本中提取工具展示对象。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 可能尚未闭合的 JSON 参数文本
///
/// 返回:
/// - 可展示对象文本
fn tool_suffix_from_partial_text(name: &str, arguments: &str) -> Option<String> {
    match name {
        "run_command" => lenient_string_field(arguments, "command").map(command_summary),
        "edit_file" | "trash_path" => {
            string_field_from_partial(arguments, &["path"]).map(file_basename)
        }
        "read_file" => read_file_suffix_from_partial(arguments),
        "glob" | "find_files" | "grep" | "search_text" => {
            string_field_from_partial(arguments, &["include", "pattern"]).map(compact_text)
        }
        "subagent" => subagent_suffix_from_partial(arguments),
        "todo" | "cron" => action_suffix_from_partial(arguments),
        "load" => load_suffix_from_partial(arguments),
        _ => None,
    }
}

/// 提取命令首个非空行作为单行展示摘要。
///
/// 参数:
/// - `value`: 原始命令文本
///
/// 返回:
/// - 压缩后的首行摘要
fn command_summary(value: String) -> String {
    let first_line = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    compact_text(first_line.to_string())
}

/// 从可能未闭合的 JSON 片段中宽松提取字符串字段。
///
/// 与严格版不同：字符串尚未闭合时返回已收到的内容，
/// 供参数流式阶段的单行状态提前展示命令。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 字段内容；未找到字段时返回空
pub(crate) fn lenient_string_field(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let value = after_colon.strip_prefix('"')?;
    let mut output = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => break,
            other => output.push(other),
        }
    }
    (!output.trim().is_empty()).then_some(output)
}

/// 提取待办或定时任务动作与对象。
fn action_suffix(arguments: &Value) -> Option<String> {
    let action = string_field(arguments, &["action"])?;
    let object = string_field(arguments, &["text", "name", "id"]);
    Some(compact_text(match object {
        Some(object) => format!("{action} {object}"),
        None => action,
    }))
}

/// 从未闭合参数中提取待办或定时任务动作。
fn action_suffix_from_partial(arguments: &str) -> Option<String> {
    let action = string_field_from_partial(arguments, &["action"])?;
    let object = string_field_from_partial(arguments, &["text", "name", "id"]);
    Some(compact_text(match object {
        Some(object) => format!("{action} {object}"),
        None => action,
    }))
}

/// 提取读取文件的展示对象。
///
/// 参数:
/// - `arguments`: 工具参数
///
/// 返回:
/// - 读取对象文本
fn read_file_suffix(arguments: &Value) -> Option<String> {
    if let Some(path) = string_field(arguments, &["path"]).map(file_basename) {
        return Some(path);
    }
    let files = arguments
        .get("files")
        .and_then(Value::as_array)
        .filter(|files| !files.is_empty())?;
    let names = files
        .iter()
        .filter_map(|file| string_field(file, &["path"]))
        .map(file_basename)
        .take(4)
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }
    let suffix = if files.len() > names.len() {
        format!("{} ...", names.join(" "))
    } else {
        names.join(" ")
    };
    Some(compact_text(suffix))
}

/// 从不完整参数文本中提取读取文件的展示对象。
///
/// 参数:
/// - `arguments`: 可能尚未闭合的 JSON 参数文本
///
/// 返回:
/// - 读取对象文本
fn read_file_suffix_from_partial(arguments: &str) -> Option<String> {
    string_field_from_partial(arguments, &["path"]).map(file_basename)
}

/// 提取子智能体展示对象。
///
/// 参数:
/// - `arguments`: 工具参数
///
/// 返回:
/// - 子智能体展示文本
fn subagent_suffix(arguments: &Value) -> Option<String> {
    let action = string_field(arguments, &["action"]).unwrap_or_else(|| "start".to_string());
    if action == "start" {
        return string_field(arguments, &["description"]).map(compact_text);
    }
    let target = string_field(arguments, &["subagent_id"])
        .map(compact_text)
        .unwrap_or_else(|| action.clone());
    Some(format!("{action} {target}"))
}

/// 从不完整参数文本中提取子智能体展示对象。
///
/// 参数:
/// - `arguments`: 可能尚未闭合的 JSON 参数文本
///
/// 返回:
/// - 子智能体展示文本
fn subagent_suffix_from_partial(arguments: &str) -> Option<String> {
    let action =
        string_field_from_partial(arguments, &["action"]).unwrap_or_else(|| "start".to_string());
    if action == "start" {
        return string_field_from_partial(arguments, &["description"]).map(compact_text);
    }
    let target = string_field_from_partial(arguments, &["subagent_id"])
        .map(compact_text)
        .unwrap_or_else(|| action.clone());
    Some(format!("{action} {target}"))
}

/// 提取加载请求的展示对象。
///
/// 参数:
/// - `arguments`: 工具参数
///
/// 返回:
/// - 加载对象文本
fn load_suffix(arguments: &Value) -> Option<String> {
    let kind = string_field(arguments, &["type", "kind"])?;
    let keywords = arguments.get("keywords").and_then(Value::as_array)?;
    let first = keywords
        .iter()
        .find_map(Value::as_str)
        .map(ToString::to_string)
        .map(compact_text)?;
    Some(format!("{} {first}", kind.to_ascii_lowercase()))
}

/// 从不完整参数文本中提取加载请求的展示对象。
///
/// 参数:
/// - `arguments`: 可能尚未闭合的 JSON 参数文本
///
/// 返回:
/// - 加载对象文本
fn load_suffix_from_partial(arguments: &str) -> Option<String> {
    let kind = string_field_from_partial(arguments, &["type", "kind"])?;
    let keyword = first_string_array_item_from_partial(arguments, "keywords")
        .or_else(|| string_field_from_partial(arguments, &["keyword"]))?;
    Some(format!(
        "{} {}",
        kind.to_ascii_lowercase(),
        compact_text(keyword)
    ))
}

/// 从可能未闭合的 JSON 数组字段中读取首个字符串。
///
/// 参数:
/// - `raw`: 流式 JSON 参数片段
/// - `key`: 数组字段名
///
/// 返回:
/// - 首个非空字符串，数组尚未闭合时也可返回已经接收的内容
fn first_string_array_item_from_partial(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let value = after_colon
        .strip_prefix('[')?
        .trim_start()
        .strip_prefix('"')?;
    let mut output = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => break,
            other => output.push(other),
        }
    }
    (!output.trim().is_empty()).then_some(output)
}

/// 从 JSON 中读取第一个非空字符串字段。
///
/// 参数:
/// - `value`: JSON 值
/// - `keys`: 待检查字段名
///
/// 返回:
/// - 字符串字段值
fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(key).and_then(Value::as_str))
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToString::to_string)
}

/// 从不完整 JSON 文本中读取第一个完整字符串字段。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `keys`: 待检查字段名
///
/// 返回:
/// - 字符串字段值
fn string_field_from_partial(raw: &str, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| json_string_field_from_partial(raw, key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 从 JSON 片段中读取指定字符串字段。
///
/// 参数:
/// - `raw`: JSON 参数片段
/// - `key`: 字段名
///
/// 返回:
/// - 字段字符串值
fn json_string_field_from_partial(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let quote_index = after_colon.find('"')?;
    let value = &after_colon[quote_index..];
    let end_index = find_json_string_end(value)?;
    serde_json::from_str::<String>(&value[..=end_index]).ok()
}

/// 查找 JSON 字符串结束位置。
///
/// 参数:
/// - `value`: 以双引号开头的 JSON 字符串片段
///
/// 返回:
/// - 结束双引号的字节位置
fn find_json_string_end(value: &str) -> Option<usize> {
    if !value.starts_with('"') {
        return None;
    }
    let mut escaped = false;
    for (index, ch) in value.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(index);
        }
    }
    None
}

/// 提取路径末尾文件名。
///
/// 参数:
/// - `value`: 路径文本
///
/// 返回:
/// - 文件名或原始路径文本
fn file_basename(value: String) -> String {
    Path::new(&value)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or(value)
}

/// 压缩展示对象文本。
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::terminal_text as t;

    #[test]
    fn command_tools_use_run_label() {
        assert_eq!(
            tool_event_label("run_command", Some(r#"{"command":"date"}"#)),
            "Run date"
        );
        // 参数尚未闭合时宽松提取已收到的命令内容
        assert_eq!(
            tool_event_label("run_command", Some(r#"{"command":"cargo bui"#)),
            "Run cargo bui"
        );
        // 多行命令只展示首个非空行
        assert_eq!(
            tool_event_label(
                "run_command",
                Some(r#"{"command":"cargo test\ncargo build"}"#)
            ),
            "Run cargo test"
        );
        assert_eq!(
            tool_event_label(
                "background_command",
                Some(r#"{"action":"start","command":"sleep 1"}"#)
            ),
            format!("{} sleep 1", t("Background start", "启动后台命令"))
        );
        assert_eq!(
            tool_event_label("background_command", Some(r#"{"action":"list"}"#)),
            t("Background list", "后台命令列表")
        );
    }

    #[test]
    fn file_tools_include_basename() {
        assert_eq!(
            tool_event_label("edit_file", Some(r#"{"path":"src/render/stream.rs"}"#)),
            "Edit stream.rs"
        );
        assert_eq!(
            tool_event_label(
                "read_file",
                Some(r#"{"files":[{"path":"src/a.rs"},{"path":"src/b.rs"}]}"#)
            ),
            "Read a.rs b.rs"
        );
    }

    #[test]
    fn partial_arguments_extract_target() {
        assert_eq!(
            tool_event_label(
                "edit_file",
                Some(r#"{"path":"src/main.rs","content":"unfinished"#)
            ),
            "Edit main.rs"
        );
        assert_eq!(
            tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_search"#)),
            "Load tool web_search"
        );
    }

    #[test]
    fn load_uses_load_label() {
        assert_eq!(
            tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_search"]}"#)),
            "Load tool web_search"
        );
        assert_eq!(
            tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_fetch"]}"#)),
            "Load tool web_fetch"
        );
        assert_eq!(
            tool_event_label("load", Some(r#"{"type":"skill","keywords":["yce"]}"#)),
            "Load skill yce"
        );
    }

    #[test]
    fn subagent_uses_description_label() {
        assert_eq!(
            tool_event_label("subagent", Some(r#"{"description":"scan code"}"#)),
            "Subagent scan code"
        );
        assert_eq!(
            tool_event_label(
                "subagent",
                Some(r#"{"action":"status","subagent_id":"subagent_1"}"#)
            ),
            "Subagent status subagent_1"
        );
    }

    #[test]
    fn management_tools_include_action_and_target() {
        assert_eq!(
            tool_event_label("todo", Some(r#"{"action":"add","text":"检查测试"}"#)),
            "Todo add 检查测试"
        );
        assert_eq!(
            tool_event_label("cron", Some(r#"{"action":"remove","id":"cron_1"}"#)),
            "Schedule remove cron_1"
        );
    }

    #[test]
    fn event_text_omits_tool_prefix() {
        let output = tool_event_text("Write main.rs", "ok");

        assert!(output.starts_with("• Write main.rs "));
        assert!(!output.contains("tool:"));
    }

    #[test]
    fn unknown_tools_use_tool_label() {
        let label = tool_event_label("custom_tool", Some(r#"{"value":1}"#));
        let output = tool_event_text(&label, "err");

        assert_eq!(label, "Tool custom_tool");
        assert!(output.contains("Tool custom_tool"));
        assert!(output.contains("\x1b[31merr\x1b[0m"));
    }
}

use serde_json::Value;
use std::path::Path;

/// 解析工具参数 JSON。
///
/// 参数:
/// - `arguments`: 工具参数 JSON 文本
///
/// 返回:
/// - 解析后的 JSON 值
pub(super) fn parse_arguments(arguments: &str) -> Option<Value> {
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
pub(super) fn tool_suffix_from_text(name: &str, arguments: &str) -> Option<String> {
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
pub(super) fn tool_suffix(name: &str, arguments: &Value) -> Option<String> {
    match name {
        "run_command" => string_field(arguments, &["command"]).map(command_summary),
        "edit_file" => patch_file_basename(arguments),
        "write_file" | "str_replace" | "trash_path" => {
            string_field(arguments, &["path"]).map(file_basename)
        }
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
pub(super) fn tool_suffix_from_partial_text(name: &str, arguments: &str) -> Option<String> {
    match name {
        "run_command" => lenient_string_field(arguments, "command").map(command_summary),
        "edit_file" => lenient_string_field(arguments, "patch").and_then(|patch| {
            patch
                .lines()
                .find_map(patch_path_from_line)
                .map(file_basename)
        }),
        "write_file" | "str_replace" | "trash_path" => {
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
pub(super) fn command_summary(value: String) -> String {
    let first_line = value
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    compact_text(first_line.to_string())
}

/// 从 edit_file patch 参数提取首个目标文件 basename。
///
/// 参数:
/// - `arguments`: 工具参数
///
/// 返回:
/// - 文件 basename
pub(super) fn patch_file_basename(arguments: &Value) -> Option<String> {
    string_field(arguments, &["patch"]).and_then(|patch| {
        patch
            .lines()
            .find_map(patch_path_from_line)
            .map(file_basename)
    })
}

/// 从 patch 头行解析文件路径。
///
/// 参数:
/// - `line`: patch 中的一行
///
/// 返回:
/// - 文件路径
pub(super) fn patch_path_from_line(line: &str) -> Option<String> {
    let path = if let Some(rest) = line.strip_prefix("*** Add File: ") {
        Some(rest.trim())
    } else if let Some(rest) = line.strip_prefix("*** Delete File: ") {
        Some(rest.trim())
    } else if let Some(rest) = line.strip_prefix("*** Update File: ") {
        Some(rest.trim())
    } else {
        None
    }?;
    let source = path
        .split_once(" -> ")
        .map(|(value, _)| value)
        .unwrap_or(path)
        .trim();
    (!source.is_empty()).then(|| source.to_string())
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
pub(super) fn action_suffix(arguments: &Value) -> Option<String> {
    let action = string_field(arguments, &["action"])?;
    let object = string_field(arguments, &["text", "name", "id"]);
    Some(compact_text(match object {
        Some(object) => format!("{action} {object}"),
        None => action,
    }))
}

/// 从未闭合参数中提取待办或定时任务动作。
pub(super) fn action_suffix_from_partial(arguments: &str) -> Option<String> {
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
pub(super) fn read_file_suffix(arguments: &Value) -> Option<String> {
    if let Some(path) = string_field(arguments, &["path"]).map(file_basename) {
        return Some(with_read_range(
            path,
            u64_field(arguments, "offset"),
            u64_field(arguments, "limit"),
        ));
    }
    let files = arguments
        .get("files")
        .and_then(Value::as_array)
        .filter(|files| !files.is_empty())?;
    let names = files
        .iter()
        .filter_map(|file| {
            let path = string_field(file, &["path"]).map(file_basename)?;
            Some(with_read_range(
                path,
                u64_field(file, "offset"),
                u64_field(file, "limit"),
            ))
        })
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
pub(super) fn read_file_suffix_from_partial(arguments: &str) -> Option<String> {
    let path = string_field_from_partial(arguments, &["path"]).map(file_basename)?;
    Some(with_read_range(
        path,
        u64_field_from_partial(arguments, "offset"),
        u64_field_from_partial(arguments, "limit"),
    ))
}

/// 将读取参数转成首末行号：`file.rs:12–91`。
///
/// 参数: `path` 为文件名，`offset` 为起始行，`limit` 为请求行数
/// 返回: 可读的文件范围；未指定范围时只保留文件名
pub(super) fn with_read_range(path: String, offset: Option<u64>, limit: Option<u64>) -> String {
    match (
        offset.filter(|value| *value > 0),
        limit.filter(|value| *value > 0),
    ) {
        (None, None) => path,
        (Some(start), Some(1)) => format!("{path}:{start}"),
        (Some(start), Some(count)) => format!("{path}:{start}–{}", start.saturating_add(count - 1)),
        (Some(start), None) => format!("{path}:{start}…"),
        (None, Some(1)) => format!("{path}:1"),
        (None, Some(count)) => format!("{path}:1–{count}"),
    }
}

/// 读取 JSON 对象上的非负整数字段。
pub(super) fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_i64().and_then(|number| u64::try_from(number).ok()))
    })
}

/// 从未闭合 JSON 片段中读取非负整数字段。
pub(super) fn u64_field_from_partial(raw: &str, key: &str) -> Option<u64> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let digits = after_key[colon_index + 1..].trim_start();
    let end = digits
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(digits.len());
    digits.get(..end)?.parse().ok()
}

/// 提取子智能体展示对象。
///
/// 参数:
/// - `arguments`: 工具参数
///
/// 返回:
/// - 子智能体展示文本
pub(super) fn subagent_suffix(arguments: &Value) -> Option<String> {
    let action = string_field(arguments, &["action"]).unwrap_or_else(|| "start".to_string());
    if action == "start" {
        return string_field(arguments, &["description"]).map(compact_text);
    }
    // action 已经由动词表达，这里只给操作对象，避免出现「Delegating wait subagent_x」
    string_field(arguments, &["subagent_id"]).map(compact_text)
}

/// 从不完整参数文本中提取子智能体展示对象。
///
/// 参数:
/// - `arguments`: 可能尚未闭合的 JSON 参数文本
///
/// 返回:
/// - 子智能体展示文本
pub(super) fn subagent_suffix_from_partial(arguments: &str) -> Option<String> {
    let action =
        string_field_from_partial(arguments, &["action"]).unwrap_or_else(|| "start".to_string());
    if action == "start" {
        return string_field_from_partial(arguments, &["description"]).map(compact_text);
    }
    string_field_from_partial(arguments, &["subagent_id"]).map(compact_text)
}

/// 提取加载请求的展示对象。
///
/// 参数:
/// - `arguments`: 工具参数
///
/// 返回:
/// - 加载对象文本
pub(super) fn load_suffix(arguments: &Value) -> Option<String> {
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
pub(super) fn load_suffix_from_partial(arguments: &str) -> Option<String> {
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
pub(super) fn first_string_array_item_from_partial(raw: &str, key: &str) -> Option<String> {
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
pub(super) fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
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
pub(super) fn string_field_from_partial(raw: &str, keys: &[&str]) -> Option<String> {
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
pub(super) fn json_string_field_from_partial(raw: &str, key: &str) -> Option<String> {
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
pub(super) fn find_json_string_end(value: &str) -> Option<usize> {
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
pub(super) fn file_basename(value: String) -> String {
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
pub(super) fn compact_text(value: String) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    // 按显示列数截断：中文路径按字符数截断会撑到近两倍宽
    crate::render::clip_to_width(&value, 48, "...")
}

use crate::render::background_command_event::background_command_call_label;
use crate::render::status_style::{color_status, tool_bullet, ToolHealth};
use crate::render::style::TOOL_BULLET;
use serde_json::Value;
use std::path::Path;

/// 工具卡动词时态：进行中 `-ing`，完成后过去式 `-ed` / 不规则过去式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ToolVerbTense {
    /// Writing / Running / Reading
    Progressive,
    /// Wrote / Ran / Read
    Perfect,
}

impl ToolVerbTense {
    /// 由工具是否已结束推导时态。
    ///
    /// 参数:
    /// - `done`: 是否已有最终结果（成功或失败）
    ///
    /// 返回:
    /// - 对应时态
    pub(crate) fn from_done(done: bool) -> Self {
        if done {
            Self::Perfect
        } else {
            Self::Progressive
        }
    }
}

/// 生成工具调用展示标签（默认进行时，供参数流/未定稿路径）。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 工具参数 JSON 文本
///
/// 返回:
/// - 面向终端展示的短标签
pub(crate) fn tool_event_label(name: &str, arguments: Option<&str>) -> String {
    tool_event_label_tense(name, arguments, ToolVerbTense::Progressive)
}

/// 生成带时态的工具调用展示标签。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 工具参数 JSON 文本
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 面向终端展示的短标签
pub(crate) fn tool_event_label_tense(
    name: &str,
    arguments: Option<&str>,
    tense: ToolVerbTense,
) -> String {
    if name == "background_command" {
        return background_command_call_label(arguments);
    }
    if name == "subagent" {
        return subagent_call_label(arguments, tense);
    }
    let action = tool_verb(name, tense);
    let suffix = arguments.and_then(|arguments| tool_suffix_from_text(name, arguments));
    match suffix {
        Some(suffix) if !suffix.trim().is_empty() => format!("{action} {suffix}"),
        _ if is_builtin_tool_verb(name) => action.to_string(),
        _ => format!("{action} {name}"),
    }
}

/// 按 action 生成子智能体工具的展示标签。
///
/// subagent 是多 action 工具，只有 start 属于「委派」。此前所有 action 共用
/// Delegating，于是 wait / status / cancel 也被显示成委派，和真正的委派混在
/// 同一串列表里分不出区别。
///
/// 参数:
/// - `arguments`: 工具参数 JSON，可能尚未闭合
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 面向终端展示的短标签
fn subagent_call_label(arguments: Option<&str>, tense: ToolVerbTense) -> String {
    let action = arguments
        .and_then(|arguments| {
            parse_arguments(arguments)
                .and_then(|value| string_field(&value, &["action"]))
                .or_else(|| string_field_from_partial(arguments, &["action"]))
        })
        .unwrap_or_else(|| "start".to_string());
    let verb = subagent_verb(&action, tense);
    let suffix = arguments.and_then(|arguments| tool_suffix_from_text("subagent", arguments));
    match suffix {
        Some(suffix) if !suffix.trim().is_empty() => format!("{verb} {suffix}"),
        _ => verb.to_string(),
    }
}

/// 子智能体各 action 对应的展示动词。
///
/// 参数:
/// - `action`: 子智能体工具 action
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 展示动词
fn subagent_verb(action: &str, tense: ToolVerbTense) -> &'static str {
    match (action, tense) {
        ("wait", ToolVerbTense::Progressive) => "Awaiting",
        ("wait", ToolVerbTense::Perfect) => "Awaited",
        ("status", ToolVerbTense::Progressive) => "Checking",
        ("status", ToolVerbTense::Perfect) => "Checked",
        ("result", ToolVerbTense::Progressive) => "Reading",
        ("result", ToolVerbTense::Perfect) => "Read",
        ("list", ToolVerbTense::Progressive) => "Listing",
        ("list", ToolVerbTense::Perfect) => "Listed",
        ("cancel", ToolVerbTense::Progressive) => "Cancelling",
        ("cancel", ToolVerbTense::Perfect) => "Cancelled",
        ("stop", ToolVerbTense::Progressive) => "Stopping",
        ("stop", ToolVerbTense::Perfect) => "Stopped",
        ("send", ToolVerbTense::Progressive) => "Messaging",
        ("send", ToolVerbTense::Perfect) => "Messaged",
        (_, tense) => tool_verb("subagent", tense),
    }
}

/// 提取命令类工具的完整命令文本（不做省略）。
///
/// 参数:
/// - `name`: 工具名
/// - `arguments`: 工具参数 JSON
///
/// 返回:
/// - 完整命令字符串；非命令工具或解析失败返回 None
pub(crate) fn tool_command_full_text(name: &str, arguments: Option<&str>) -> Option<String> {
    let arguments = arguments?;
    match name {
        "run_command" => parse_arguments(arguments)
            .and_then(|value| string_field(&value, &["command"]))
            .or_else(|| lenient_string_field(arguments, "command"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        "background_command" => {
            let action = parse_arguments(arguments)
                .and_then(|value| string_field(&value, &["action"]))
                .or_else(|| lenient_string_field(arguments, "action"))
                .unwrap_or_else(|| "start".to_string());
            let command = parse_arguments(arguments)
                .and_then(|value| string_field(&value, &["command"]))
                .or_else(|| lenient_string_field(arguments, "command"));
            match command {
                Some(command) if !command.trim().is_empty() => {
                    Some(format!("{action} {}", command.trim()))
                }
                _ if !action.trim().is_empty() => Some(action),
                _ => None,
            }
        }
        _ => None,
    }
}

/// 渲染带 shell 语法着色的命令标题（完整命令不省略）。
///
/// 参数:
/// - `name`: 工具名
/// - `arguments`: 工具参数 JSON
///
/// 返回:
/// - ANSI 标题；非命令工具时回退到短标签
pub(crate) fn tool_command_title_colored(name: &str, arguments: Option<&str>) -> String {
    tool_command_title_colored_tense(name, arguments, ToolVerbTense::Progressive)
}

/// 渲染带时态与 shell 语法着色的命令标题。
///
/// 参数:
/// - `name`: 工具名
/// - `arguments`: 工具参数 JSON
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - ANSI 标题；非命令工具时回退到短标签
pub(crate) fn tool_command_title_colored_tense(
    name: &str,
    arguments: Option<&str>,
    tense: ToolVerbTense,
) -> String {
    let action = if name == "background_command" {
        "Background".to_string()
    } else {
        tool_verb(name, tense).to_string()
    };
    if let Some(command) = tool_command_full_text(name, arguments) {
        // 多行命令逐行着色，保留完整文本
        let colored = command
            .lines()
            .map(|line| crate::render::code_block::highlight_code_line("bash", line))
            .collect::<Vec<_>>()
            .join("\n");
        return format!("{action} {colored}");
    }
    tool_event_label_tense(name, arguments, tense)
}

/// 生成工具状态事件行（transcript / 定稿路径的统一排版）。
///
/// 统一层级：状态色圆点 + 粗体动词 + 常规对象 + 语义色徽标。
/// 状态语义由状态键自动推导；编辑类 `+N -M` 等自定义徽标需要
/// 显式语义时请使用 `tool_status_line`。
///
/// 参数:
/// - `label`: 工具展示标签（`动词 对象`）
/// - `status`: 状态键（ok/err/run/arg/skip）或自定义徽标
///
/// 返回:
/// - 单行 ANSI 状态文本
pub(crate) fn tool_event_text(label: &str, status: &str) -> String {
    tool_status_line(
        label,
        &color_status(status),
        ToolHealth::from_status(status),
    )
}

/// 以显式状态语义生成工具状态事件行。
///
/// 参数:
/// - `label`: 工具展示标签（`动词 对象`）
/// - `badge`: 行尾徽标（可含 ANSI，如 `+N -M`；空则省略）
/// - `health`: 状态语义，决定行首圆点颜色
///
/// 返回:
/// - 单行 ANSI 状态文本
pub(crate) fn tool_status_line(label: &str, badge: &str, health: ToolHealth) -> String {
    let bullet = tool_bullet(health);
    let title = emphasize_verb(label);
    if badge.is_empty() {
        return format!("{bullet} {title}");
    }
    format!("{bullet} {title} {badge}")
}

/// 加粗标签首词（动词），保持对象部分常规色。
///
/// 首词已含 ANSI 样式时原样返回，避免破坏调用方的自定义着色；
/// 对象部分的 ANSI（如 todo 状态符）不影响动词加粗。
///
/// 参数:
/// - `label`: 展示标签
///
/// 返回:
/// - 动词加粗后的标签
fn emphasize_verb(label: &str) -> String {
    match label.split_once(' ') {
        Some((verb, rest)) if !verb.contains('\x1b') => format!("\x1b[1m{verb}\x1b[0m {rest}"),
        None if !label.contains('\x1b') => format!("\x1b[1m{label}\x1b[0m"),
        _ => label.to_string(),
    }
}

/// 生成流式阶段的单行工具状态（整行随后由调用方弱化）。
///
/// 与定稿行不同：不点亮圆点、不加粗动词，保持 live 行整体安静。
///
/// 参数:
/// - `label`: 工具展示标签
/// - `status`: 工具状态键
///
/// 返回:
/// - 可直接写入终端的单行状态文本
pub(crate) fn tool_call_status_text(label: &str, status: &str) -> String {
    format!("{TOOL_BULLET} {label} {}", color_status(status))
}

/// 返回工具动作动词（按时态）。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 展示用动词
pub(crate) fn tool_verb(name: &str, tense: ToolVerbTense) -> &'static str {
    match (name, tense) {
        ("run_command", ToolVerbTense::Progressive) => "Running",
        ("run_command", ToolVerbTense::Perfect) => "Ran",
        ("edit_file", ToolVerbTense::Progressive) => "Editing",
        ("edit_file", ToolVerbTense::Perfect) => "Edited",
        ("write_file", ToolVerbTense::Progressive) => "Writing",
        ("write_file", ToolVerbTense::Perfect) => "Wrote",
        ("str_replace", ToolVerbTense::Progressive) => "Replacing",
        ("str_replace", ToolVerbTense::Perfect) => "Replaced",
        ("read_file", ToolVerbTense::Progressive) => "Reading",
        ("read_file", ToolVerbTense::Perfect) => "Read",
        ("trash_path", ToolVerbTense::Progressive) => "Trashing",
        ("trash_path", ToolVerbTense::Perfect) => "Trashed",
        ("glob" | "find_files", ToolVerbTense::Progressive) => "Finding",
        ("glob" | "find_files", ToolVerbTense::Perfect) => "Found",
        ("grep" | "search_text", ToolVerbTense::Progressive) => "Searching",
        ("grep" | "search_text", ToolVerbTense::Perfect) => "Searched",
        ("subagent", ToolVerbTense::Progressive) => "Delegating",
        ("subagent", ToolVerbTense::Perfect) => "Delegated",
        ("todo", ToolVerbTense::Progressive) => "Updating",
        ("todo", ToolVerbTense::Perfect) => "Updated",
        ("cron", ToolVerbTense::Progressive) => "Scheduling",
        ("cron", ToolVerbTense::Perfect) => "Scheduled",
        ("check_os_info", ToolVerbTense::Progressive) => "Checking",
        ("check_os_info", ToolVerbTense::Perfect) => "Checked",
        ("load", ToolVerbTense::Progressive) => "Loading",
        ("load", ToolVerbTense::Perfect) => "Loaded",
        ("create_directory", ToolVerbTense::Progressive) => "Creating",
        ("create_directory", ToolVerbTense::Perfect) => "Created",
        ("list_directory", ToolVerbTense::Progressive) => "Listing",
        ("list_directory", ToolVerbTense::Perfect) => "Listed",
        (_, ToolVerbTense::Progressive) => "Running",
        (_, ToolVerbTense::Perfect) => "Ran",
    }
}

/// 将已生成标签的动词切换到目标时态，保留后缀（路径/命令摘要）。
///
/// 参数:
/// - `name`: 工具名
/// - `label`: 现有标签
/// - `tense`: 目标时态
///
/// 返回:
/// - 切换动词后的标签；无法识别前缀时原样返回
pub(crate) fn retarget_label_tense(name: &str, label: &str, tense: ToolVerbTense) -> String {
    let target = tool_verb(name, tense);
    for candidate in [ToolVerbTense::Progressive, ToolVerbTense::Perfect] {
        let verb = tool_verb(name, candidate);
        if let Some(rest) = label.strip_prefix(verb) {
            if rest.is_empty() || rest.starts_with(' ') {
                return format!("{target}{rest}");
            }
        }
    }
    label.to_string()
}

/// 是否为内置工具（有专用动词，标签不必再拼原始工具名）。
fn is_builtin_tool_verb(name: &str) -> bool {
    matches!(
        name,
        "run_command"
            | "edit_file"
            | "write_file"
            | "str_replace"
            | "read_file"
            | "trash_path"
            | "glob"
            | "find_files"
            | "grep"
            | "search_text"
            | "subagent"
            | "todo"
            | "cron"
            | "check_os_info"
            | "load"
            | "create_directory"
            | "list_directory"
    )
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
fn tool_suffix_from_partial_text(name: &str, arguments: &str) -> Option<String> {
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
fn command_summary(value: String) -> String {
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
fn patch_file_basename(arguments: &Value) -> Option<String> {
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
fn patch_path_from_line(line: &str) -> Option<String> {
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
fn read_file_suffix_from_partial(arguments: &str) -> Option<String> {
    let path = string_field_from_partial(arguments, &["path"]).map(file_basename)?;
    Some(with_read_range(
        path,
        u64_field_from_partial(arguments, "offset"),
        u64_field_from_partial(arguments, "limit"),
    ))
}

/// 把读取起点与行数接到文件名后面：`file.rs:12+80`。
///
/// 参数未带 offset/limit 时只保留文件名，避免每个整文件读取都写成 `:1+2000`。
fn with_read_range(path: String, offset: Option<u64>, limit: Option<u64>) -> String {
    match (
        offset.filter(|value| *value > 0),
        limit.filter(|value| *value > 0),
    ) {
        (None, None) => path,
        (Some(start), Some(count)) => format!("{path}:{start}+{count}"),
        (Some(start), None) => format!("{path}:{start}+"),
        (None, Some(count)) => format!("{path}:1+{count}"),
    }
}

/// 读取 JSON 对象上的非负整数字段。
fn u64_field(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_i64().and_then(|number| u64::try_from(number).ok()))
    })
}

/// 从未闭合 JSON 片段中读取非负整数字段。
fn u64_field_from_partial(raw: &str, key: &str) -> Option<u64> {
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
fn subagent_suffix(arguments: &Value) -> Option<String> {
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
fn subagent_suffix_from_partial(arguments: &str) -> Option<String> {
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
#[path = "tool_event_line_tests.rs"]
mod tests;

use crate::render::background_command_event::background_command_call_label_tense;
use crate::render::status_style::{color_status, tool_bullet, ToolHealth};
use crate::render::style::TOOL_BULLET;
use serde_json::Value;

mod suffix;
pub(crate) use suffix::lenient_string_field;
use suffix::*;

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
        return background_command_call_label_tense(arguments, tense);
    }
    if name == "subagent" {
        return subagent_call_label(arguments, tense);
    }
    if name == "todo" {
        return todo_call_label(arguments, tense);
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

/// 按 action 生成 todo 工具的展示标签。
///
/// todo 的 action 是面向模型的接口词，直接拼进标签会出现
/// `Updating update 补齐回归测试` 这类同义重复。这里把 action 映射为
/// 面向用户的动词，update 按是否改文本区分 Marking/Editing，
/// 结果渲染层的 `changed_item_label` 再用条目内容替换对象。
///
/// 参数:
/// - `arguments`: 工具参数 JSON，可能尚未闭合
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 面向终端展示的短标签
fn todo_call_label(arguments: Option<&str>, tense: ToolVerbTense) -> String {
    let Some(arguments) = arguments else {
        return tool_verb("todo", tense).to_string();
    };
    let action = parse_arguments(arguments)
        .and_then(|value| string_field(&value, &["action"]))
        .or_else(|| string_field_from_partial(arguments, &["action"]))
        .unwrap_or_else(|| "list".to_string());
    let verb = todo_verb(&action, arguments, tense);
    let suffix = todo_object(arguments, &action);
    match suffix {
        Some(suffix) if !suffix.trim().is_empty() => format!("{verb} {suffix}"),
        _ => verb.to_string(),
    }
}

/// todo 各 action 对应的展示动词。
///
/// 参数:
/// - `action`: todo 工具 action
/// - `arguments`: 工具参数 JSON，用于区分 update 是否改文本
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 展示动词
fn todo_verb(action: &str, arguments: &str, tense: ToolVerbTense) -> &'static str {
    match action {
        "list" => tool_verb("list_directory", tense),
        "add" => {
            if tense == ToolVerbTense::Progressive {
                "Adding"
            } else {
                "Added"
            }
        }
        "remove" => {
            if tense == ToolVerbTense::Progressive {
                "Removing"
            } else {
                "Removed"
            }
        }
        // update 只改状态时是标记，改文本时是编辑
        "update" => {
            let has_text = parse_arguments(arguments)
                .map(|value| {
                    value
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty())
                })
                .unwrap_or(false);
            match (has_text, tense) {
                (true, ToolVerbTense::Progressive) => "Editing",
                (true, ToolVerbTense::Perfect) => "Edited",
                (false, ToolVerbTense::Progressive) => "Marking",
                (false, ToolVerbTense::Perfect) => "Marked",
            }
        }
        _ => tool_verb("todo", tense),
    }
}

/// 提取 todo 展示对象：优先用条目文本，其次用状态。
///
/// 参数:
/// - `arguments`: 工具参数 JSON，可能尚未闭合
/// - `action`: todo 工具 action
///
/// 返回:
/// - 可展示对象文本
fn todo_object(arguments: &str, action: &str) -> Option<String> {
    if action == "list" {
        return None;
    }
    let parsed = parse_arguments(arguments);
    let text = parsed
        .as_ref()
        .and_then(|value| string_field(value, &["text"]))
        .or_else(|| string_field_from_partial(arguments, &["text"]));
    if let Some(text) = text {
        return Some(compact_text(text));
    }
    // add 多条 texts：取首条加省略
    let texts = parsed
        .as_ref()
        .and_then(|value| value.get("texts").and_then(Value::as_array))
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        });
    if let Some(texts) = texts {
        if texts.is_empty() {
            return None;
        }
        if texts.len() == 1 {
            return Some(compact_text(texts[0].clone()));
        }
        return Some(compact_text(format!("{} …", texts[0])));
    }
    // update 只改状态：展示目标状态
    parsed
        .as_ref()
        .and_then(|value| string_field(value, &["status"]))
        .map(compact_text)
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

#[cfg(test)]
#[path = "tool_event_line_tests.rs"]
mod tests;

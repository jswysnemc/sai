//! 补充内置工具的动词与展示对象。
//!
//! 网页搜索、浏览器、记忆、目标、SSH 等工具不在主动词表里时会落到
//! 兜底的 Running/Ran，状态行看不出在做什么。这里集中给出它们的专用
//! 动词、兜底对象与参数提取规则。

use super::suffix::{
    command_summary_text, file_basename, format_generic_object, lenient_string_field,
    parse_arguments, string_field,
};
use super::ToolVerbTense;
use serde_json::Value;

/// 浏览器各 action 的进行时与完成时动词。
const BROWSER_VERBS: &[(&str, &str, &str)] = &[
    ("navigate", "Opening", "Opened"),
    ("back", "Going back", "Went back"),
    ("forward", "Going forward", "Went forward"),
    ("reload", "Reloading", "Reloaded"),
    ("tabs", "Listing tabs", "Listed tabs"),
    ("new_tab", "Opening tab", "Opened tab"),
    ("switch_tab", "Switching tab", "Switched tab"),
    ("close_tab", "Closing tab", "Closed tab"),
    ("wait", "Waiting for", "Waited for"),
    ("snapshot", "Reading page", "Read page"),
    ("screenshot", "Capturing screenshot", "Captured screenshot"),
    ("evaluate", "Evaluating", "Evaluated"),
    ("click", "Clicking", "Clicked"),
    ("double_click", "Double-clicking", "Double-clicked"),
    ("right_click", "Right-clicking", "Right-clicked"),
    ("hover", "Hovering", "Hovered"),
    ("type", "Typing into", "Typed into"),
    ("select_option", "Selecting", "Selected"),
    ("press_key", "Pressing", "Pressed"),
    ("scroll", "Scrolling", "Scrolled"),
];

/// 返回补充工具的动作动词。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 专用动词；不是补充工具时返回空
pub(super) fn extra_tool_verb(name: &str, tense: ToolVerbTense) -> Option<&'static str> {
    let (progressive, perfect) = match name {
        "web_search" => ("Searching", "Searched"),
        "browser" => ("Browsing", "Browsed"),
        "write_memory" => ("Saving", "Saved"),
        "read_memory" => ("Recalling", "Recalled"),
        "list_memory" => ("Listing", "Listed"),
        "delete_memory" => ("Forgetting", "Forgot"),
        "search_evicted_context" => ("Searching", "Searched"),
        "create_goal" => ("Setting goal", "Set goal"),
        "get_goal" => ("Checking", "Checked"),
        "update_goal" => ("Updating goal", "Updated goal"),
        "scientific_calculator" => ("Calculating", "Calculated"),
        "send_channel_message" | "mesh_send" => ("Sending", "Sent"),
        "mcp_manager" => ("Managing", "Managed"),
        "ssh_list_hosts" => ("Listing", "Listed"),
        "ssh_run_command" => ("Running", "Ran"),
        "ssh_upload_file" => ("Uploading", "Uploaded"),
        "ssh_download_file" => ("Downloading", "Downloaded"),
        "analyze_image" => ("Analyzing", "Analyzed"),
        _ => return None,
    };
    Some(match tense {
        ToolVerbTense::Progressive => progressive,
        ToolVerbTense::Perfect => perfect,
    })
}

/// 补充工具在参数缺少对象时的兜底对象。
///
/// 参数:
/// - `name`: 工具原始名称
///
/// 返回:
/// - 兜底对象；动词本身已完整时返回空
pub(super) fn extra_fallback_object(name: &str) -> Option<&'static str> {
    match name {
        "web_search" => Some("the web"),
        "write_memory" | "read_memory" | "delete_memory" => Some("memory"),
        "list_memory" => Some("memories"),
        "search_evicted_context" => Some("earlier context"),
        "get_goal" => Some("goal"),
        "scientific_calculator" => Some("expression"),
        "send_channel_message" | "mesh_send" => Some("message"),
        "mcp_manager" => Some("MCP servers"),
        "ssh_list_hosts" => Some("SSH hosts"),
        "ssh_run_command" => Some("remote command"),
        "ssh_upload_file" | "ssh_download_file" => Some("file"),
        "analyze_image" => Some("image"),
        _ => None,
    }
}

/// 提取补充工具的展示对象，完整与不完整参数都可处理。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `arguments`: 工具参数 JSON 文本，可能尚未闭合
///
/// 返回:
/// - 展示对象；不是补充工具或参数里没有可用字段时返回空
pub(super) fn extra_tool_suffix(name: &str, arguments: &str) -> Option<String> {
    let parsed = parse_arguments(arguments);
    // 1. 按字段顺序取第一个非空字符串，完整 JSON 优先
    let field = |keys: &[&str]| {
        parsed
            .as_ref()
            .and_then(|value| string_field(value, keys))
            .or_else(|| {
                keys.iter()
                    .find_map(|key| lenient_string_field(arguments, key))
            })
    };
    // 2. 按工具选择字段与格式
    match name {
        "web_search" | "search_evicted_context" => {
            field(&["query"]).and_then(format_generic_object)
        }
        "write_memory" | "read_memory" | "delete_memory" => {
            field(&["name"]).and_then(format_generic_object)
        }
        "create_goal" => field(&["objective"]).and_then(format_generic_object),
        "update_goal" => field(&["status", "note"]).and_then(format_generic_object),
        "scientific_calculator" => field(&["expression"]).and_then(format_generic_object),
        "send_channel_message" => {
            field(&["text", "caption", "path"]).and_then(format_generic_object)
        }
        "mesh_send" => field(&["to"]).and_then(format_generic_object),
        "mcp_manager" => {
            let action = field(&["action"])?;
            Some(match field(&["server_id"]) {
                Some(server) => format!("{action} {server}"),
                None => action,
            })
        }
        "ssh_run_command" => {
            let host = field(&["host_id"]);
            let command = field(&["command"]).map(command_summary_text);
            match (host, command) {
                (Some(host), Some(command)) => Some(format!("{command} on {host}")),
                (None, Some(command)) => Some(command),
                (Some(host), None) => Some(host),
                (None, None) => None,
            }
        }
        "ssh_upload_file" => field(&["local_path"]).map(file_basename),
        "ssh_download_file" => field(&["remote_path"]).map(file_basename),
        _ => None,
    }
}

/// 生成浏览器工具标签：按 action 选动词，再拼操作对象。
///
/// 参数:
/// - `arguments`: 工具参数 JSON 文本，可能尚未闭合
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 面向终端展示的短标签
pub(super) fn browser_call_label(arguments: Option<&str>, tense: ToolVerbTense) -> String {
    let raw = arguments.unwrap_or_default();
    let parsed: Option<Value> = parse_arguments(raw);
    let field = |keys: &[&str]| {
        parsed
            .as_ref()
            .and_then(|value| string_field(value, keys))
            .or_else(|| keys.iter().find_map(|key| lenient_string_field(raw, key)))
    };
    let action = field(&["action"]).unwrap_or_default();
    let verb = browser_verb(&action, tense);
    // 1. 按 action 取对象字段；缺失时给出可读的兜底对象
    let (keys, fallback): (&[&str], Option<&str>) = match action.as_str() {
        "navigate" | "new_tab" => (&["url"], None),
        "switch_tab" | "close_tab" => (&["id"], None),
        "wait" => (&["selector"], Some("page")),
        "reload" | "scroll" => (&["direction"], Some("page")),
        "evaluate" => (&[], Some("script")),
        "click" | "double_click" | "right_click" | "hover" | "type" => {
            (&["ref", "selector"], Some("element"))
        }
        "select_option" => (&["option"], Some("option")),
        "press_key" => (&["key"], Some("key")),
        "" => (&["url"], Some("page")),
        _ => (&[], None),
    };
    let object = field(keys)
        .and_then(format_generic_object)
        .or_else(|| fallback.map(str::to_string));
    match object {
        Some(object) => format!("{verb} {object}"),
        None => verb.to_string(),
    }
}

/// 浏览器 action 对应的动词。
///
/// 参数:
/// - `action`: 浏览器工具 action
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 展示动词；未知 action 用通用的浏览动词
fn browser_verb(action: &str, tense: ToolVerbTense) -> &'static str {
    let (_, progressive, perfect) = BROWSER_VERBS
        .iter()
        .find(|(name, _, _)| *name == action)
        .copied()
        .unwrap_or(("", "Browsing", "Browsed"));
    match tense {
        ToolVerbTense::Progressive => progressive,
        ToolVerbTense::Perfect => perfect,
    }
}

/// 浏览器全部动词对，供已生成标签切换时态时识别前缀。
///
/// 返回:
/// - (进行时, 完成时) 动词对
pub(super) fn browser_verb_pairs() -> impl Iterator<Item = (&'static str, &'static str)> {
    BROWSER_VERBS
        .iter()
        .map(|(_, progressive, perfect)| (*progressive, *perfect))
}

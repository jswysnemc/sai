//! 网页搜索与文件内容搜索的状态行标签。
//!
//! 两类工具原先共用 `Searching X`，状态行分不出是在查网页还是查代码。
//! 这里给出各自的完整句式：
//! - 网页：`Searching the web for <query>`
//! - 文件：`Searching files for <pattern> in <glob>`

use super::suffix::{compact_text, lenient_string_field, parse_arguments, string_field};
use super::ToolVerbTense;
use serde_json::Value;

/// 判断工具是否使用搜索专用标签。
///
/// 参数:
/// - `name`: 工具原始名称
///
/// 返回:
/// - 网页搜索或文件内容搜索时为真
pub(super) fn is_search_tool(name: &str) -> bool {
    matches!(name, "web_search" | "grep" | "search_text")
}

/// 【终端】【搜索标签】生成网页搜索或文件内容搜索的状态行标签。
///
/// 参数:
/// - `name`: 工具原始名称，须满足 [`is_search_tool`]
/// - `arguments`: 工具参数 JSON 文本，可能尚未闭合
/// - `tense`: 进行中 / 已完成
///
/// 返回:
/// - 如 `Searched the web for rust traits` / `Searching files for fn main in *.rs`
pub(super) fn search_call_label(
    name: &str,
    arguments: Option<&str>,
    tense: ToolVerbTense,
) -> String {
    let raw = arguments.unwrap_or_default();
    let parsed = parse_arguments(raw);
    // 1. 完整 JSON 优先，参数流到一半时退回宽松提取
    let field = |key: &str| {
        parsed
            .as_ref()
            .and_then(|value: &Value| string_field(value, &[key]))
            .or_else(|| lenient_string_field(raw, key))
            .filter(|value| !value.trim().is_empty())
            .map(compact_text)
    };
    let verb = match tense {
        ToolVerbTense::Progressive => "Searching",
        ToolVerbTense::Perfect => "Searched",
    };
    // 2. 按工具拼出范围与对象
    if name == "web_search" {
        return match field("query") {
            Some(query) => format!("{verb} the web for {query}"),
            None => format!("{verb} the web"),
        };
    }
    let mut label = match field("pattern") {
        Some(pattern) => format!("{verb} files for {pattern}"),
        None => format!("{verb} files"),
    };
    if let Some(include) = field("include") {
        label.push_str(&format!(" in {include}"));
    }
    label
}

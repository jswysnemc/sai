//! 网页搜索与文件内容搜索的定稿视图。
//!
//! 两类搜索在状态行之外各挂一枚结果徽标，扫一眼即可区分：
//! - 网页：`5 results · Brave`
//! - 文件：`12 matches in 4 files`

use crate::render::status_style::ToolHealth;
use crate::render::tool_event_line::{tool_event_label_tense, tool_status_line, ToolVerbTense};
use crate::render::ToolCallDisplayMode;
use serde_json::Value;
use std::collections::BTreeSet;

/// 【终端】【搜索视图】渲染定稿后的搜索状态行（Summary 模式）。
///
/// 只接管成功结束的 Summary 视图；进行中、失败或 Full 模式交回通用路径，
/// 保留参数与输出载荷。
///
/// 参数:
/// - `view`: 搜索工具生命周期
/// - `mode`: 工具展示模式
///
/// 返回:
/// - ANSI 状态行；不归本视图处理时返回空
pub(crate) fn render(view: &super::model::ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if mode != ToolCallDisplayMode::Summary {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let badge = match view.name.as_str() {
        "web_search" => web_results_badge(&outcome.output),
        "grep" | "search_text" => file_matches_badge(&outcome.output),
        _ => return None,
    }?;
    let label = tool_event_label_tense(&view.name, Some(&view.arguments), ToolVerbTense::Perfect);
    let mut output = tool_status_line(&label, &format!("\x1b[2m{badge}\x1b[0m"), ToolHealth::Ok);
    output.push_str(&super::formatter::render_permission(
        view.permission.as_ref(),
    ));
    Some(output)
}

/// 统计网页搜索结果条数与供应商。
///
/// 结果为 Markdown：`**Provider**: X` 一行，每条结果以 `### N.` 开头。
///
/// 参数:
/// - `output`: web_search 工具输出
///
/// 返回:
/// - 如 `5 results · Brave`；不是搜索结果格式时返回空
fn web_results_badge(output: &str) -> Option<String> {
    let mut provider = None;
    let mut count = 0usize;
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("**Provider**:") {
            provider = Some(rest.trim().to_string());
        } else if line.starts_with("### ") {
            count += 1;
        }
    }
    let provider = provider?;
    let noun = if count == 1 { "result" } else { "results" };
    if provider.is_empty() {
        Some(format!("{count} {noun}"))
    } else {
        Some(format!("{count} {noun} · {provider}"))
    }
}

/// 统计文件内容搜索的命中行数与文件数。
///
/// 结果为 JSON，`stdout` 每行形如 `path:line:text`（单文件搜索时为 `line:text`）。
///
/// 参数:
/// - `output`: grep 工具输出
///
/// 返回:
/// - 如 `12 matches in 4 files` / `no matches`；无法解析时返回空
fn file_matches_badge(output: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(output.trim()).ok()?;
    let stdout = value.get("stdout")?.as_str()?;
    let truncated = value
        .get("truncated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // 1. 逐行收集命中与所属文件
    let mut matches = 0usize;
    let mut files = BTreeSet::new();
    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        matches += 1;
        if let Some(path) = match_path(line) {
            files.insert(path);
        }
    }
    if matches == 0 {
        return Some("no matches".to_string());
    }
    // 2. 截断时数量只是下限，用 `+` 标出
    let plus = if truncated { "+" } else { "" };
    let match_noun = if matches == 1 { "match" } else { "matches" };
    if files.is_empty() {
        return Some(format!("{matches}{plus} {match_noun}"));
    }
    let file_noun = if files.len() == 1 { "file" } else { "files" };
    Some(format!(
        "{matches}{plus} {match_noun} in {} {file_noun}",
        files.len()
    ))
}

/// 从 `path:line:text` 中取出文件路径。
///
/// 参数:
/// - `line`: 单行搜索结果
///
/// 返回:
/// - 文件路径；行首就是行号（单文件搜索）时返回空
fn match_path(line: &str) -> Option<&str> {
    let mut parts = line.splitn(3, ':');
    let path = parts.next()?;
    let number = parts.next()?;
    if path.is_empty()
        || path.chars().all(|ch| ch.is_ascii_digit())
        || !number.chars().all(|ch| ch.is_ascii_digit())
        || number.is_empty()
    {
        return None;
    }
    Some(path)
}

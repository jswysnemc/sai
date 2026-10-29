use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【路径结果】文件查找保留匹配范围与路径，隐藏正常退出码及默认限制。
/// 参数: 工具视图和模式；返回: 路径列表或空值。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if !matches!(view.name.as_str(), "glob" | "find_files") {
        return None;
    }
    let outcome = view.outcome.as_ref()?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    let stdout = value["stdout"].as_str()?;
    let args: Value = serde_json::from_str(&view.arguments).unwrap_or_default();
    let count = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let success = outcome.ok
        && value["ok"] != false
        && value["success"] != false
        && value["exit_code"].as_i64().is_none_or(|code| code == 0);
    let truncated = value["truncated"] == true;
    let mut title = if success {
        format!(
            "Matched {count}{} {}",
            if truncated { "+" } else { "" },
            if count == 1 { "file" } else { "files" }
        )
    } else {
        "File search failed".into()
    };
    if let Some(pattern) = result_card::text(&args, "pattern") {
        title.push_str(&format!(" · {}", result_card::short(&pattern, 52)));
    }
    let mut body = Vec::new();
    if let Some(path) = result_card::text(&args, "path") {
        body.push(format!("In {path}"));
    }
    body.extend(stdout.lines().map(crate::render::ansi_clip::strip_ansi));
    if truncated {
        body.push("Results truncated; narrow the search to see more.".into());
    }
    if let Some(error) = result_card::text(&value, "stderr") {
        body.push(error);
    }
    if !success {
        if let Some(code) = value["exit_code"].as_i64() {
            body.push(format!("Exit code: {code}"));
        }
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        if success {
            ToolHealth::Ok
        } else {
            ToolHealth::Err
        },
        &body.join("\n"),
    ))
}

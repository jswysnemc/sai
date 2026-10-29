use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;
use std::collections::BTreeSet;

/// 【终端】【文件搜索】以命中摘要和定位行呈现本地搜索，不展示成功协议字段。
/// 参数: 工具视图及显示模式；返回: 可识别结果的专用卡片。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if !matches!(view.name.as_str(), "grep" | "search_text") {
        return None;
    }
    let outcome = view.outcome.as_ref()?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    let stdout = value.get("stdout")?.as_str()?;
    let args: Value = serde_json::from_str(&view.arguments).unwrap_or_default();
    let query = result_card::text(&args, "pattern")
        .unwrap_or_default()
        .replace(['\n', '\r', '\t'], " ");
    let success = outcome.ok
        && value["ok"] != false
        && value["success"] != false
        && value["exit_code"]
            .as_i64()
            .is_none_or(|code| code == 0 || (code == 1 && stdout.is_empty()));
    let truncated = value["truncated"] == true;
    let matches = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let files = matches
        .iter()
        .filter_map(|line| location(line).map(|(path, _, _)| path))
        .filter(|path| !path.is_empty())
        .collect::<BTreeSet<_>>();
    let count = matches.len();
    let badge = if count == 0 {
        if truncated {
            "no matches in partial results"
        } else {
            "no matches"
        }
        .into()
    } else {
        let mut badge = format!(
            "{count}{} {}",
            if truncated { "+" } else { "" },
            if count == 1 { "match" } else { "matches" }
        );
        if !files.is_empty() {
            badge.push_str(&format!(
                " in {} {}",
                files.len(),
                if files.len() == 1 { "file" } else { "files" }
            ));
        }
        badge
    };
    let action = if success {
        "Searched files"
    } else {
        "Search failed"
    };
    let query = if query.is_empty() {
        String::new()
    } else {
        format!(" · \"{query}\"")
    };
    let title = format!("{action} · {badge}{query}");
    let mut body = Vec::new();
    if let Some(path) = result_card::text(&args, "path") {
        body.push(format!("\x1b[2mScope: {path}\x1b[0m"));
    }
    if let Some(include) = result_card::text(&args, "include") {
        body.push(format!("\x1b[2mFilter: {include}\x1b[0m"));
    }
    // 1. 【终端】【搜索定位】路径与行号独占定位行，匹配正文保留原始文本和冒号
    for line in matches {
        if let Some((path, number, content)) = location(line) {
            let place = if path.is_empty() {
                format!("Line {number}")
            } else {
                format!("{path}:{number}")
            };
            body.push(format!(
                "\x1b[36m{}\x1b[0m\n  {}",
                crate::render::ansi_clip::strip_ansi(&place),
                crate::render::ansi_clip::strip_ansi(content)
            ));
        } else {
            body.push(crate::render::ansi_clip::strip_ansi(line));
        }
    }
    // 2. 【终端】【搜索诊断】只在需要时呈现截断、错误输出和异常退出码
    if truncated {
        body.push(
            "\x1b[33mResults truncated; narrow the search or increase max_results.\x1b[0m".into(),
        );
    }
    if let Some(error) = result_card::text(&value, "stderr") {
        body.push(format!("\x1b[31m{error}\x1b[0m"));
    }
    if !success {
        body.push(format!("Exit code: {}", value["exit_code"]));
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

/// 【终端】【搜索定位】识别文件或单文件命中格式，兼容 Windows 盘符与正文冒号。
/// 参数: 命中行；返回: 路径、行号及正文切片，无法识别时返回空。
fn location(line: &str) -> Option<(&str, &str, &str)> {
    for (position, _) in line.match_indices(':') {
        let before = &line[..position];
        if !before.is_empty() && before.bytes().all(|ch| ch.is_ascii_digit()) {
            return Some(("", before, &line[position + 1..]));
        }
        let (number, content) = line[position + 1..].split_once(':')?;
        if !number.is_empty() && number.bytes().all(|ch| ch.is_ascii_digit()) {
            return Some((before, number, content));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 【终端】【搜索定位测试】盘符与正文冒号不改变命中位置；无参数和返回值。
    #[test]
    fn location_keeps_colons() {
        assert_eq!(
            location("C:\\src\\a.rs:12:let x: i32"),
            Some(("C:\\src\\a.rs", "12", "let x: i32"))
        );
        assert_eq!(location("12:some:text"), Some(("", "12", "some:text")));
    }
}

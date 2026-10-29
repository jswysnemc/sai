use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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
    let pattern = args["pattern"].as_str().unwrap_or_default();
    let query = crate::render::ansi_clip::strip_ansi(pattern).replace(['\n', '\r', '\t'], " ");
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
    if success && mode == ToolCallDisplayMode::Summary {
        return Some(result_card::render(view, mode, &title, ToolHealth::Ok, ""));
    }
    let matcher = regex::Regex::new(pattern).ok();
    let mut body = Vec::new();
    if let Some(path) = result_card::text(&args, "path") {
        body.push(format!("\x1b[2mScope: {path}\x1b[0m"));
    }
    if let Some(include) = result_card::text(&args, "include") {
        body.push(format!("\x1b[2mFilter: {include}\x1b[0m"));
    }
    // 1. 【终端】【搜索定位】按文件分组，行号和高亮正文构成第二级，保留正文冒号
    let folded = crate::render::fold_text::fold_preview_lines(
        &matches
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>(),
        false,
    );
    let mut grouped = BTreeMap::<&str, Vec<(&str, &str)>>::new();
    for line in &folded {
        let line = match line {
            crate::render::fold_text::FoldedDisplayLine::Line(line) => line.as_str(),
            crate::render::fold_text::FoldedDisplayLine::Omitted { omitted, .. } => {
                body.push(crate::render::omitted_line::render_fold_hint(
                    &format!("{omitted} matches hidden"),
                    Some(crate::render::omitted_line::FOLD_SHORTCUT),
                ));
                continue;
            }
        };
        if let Some((path, number, content)) = location(line) {
            grouped.entry(path).or_default().push((number, content));
        } else {
            body.push(crate::render::ansi_clip::strip_ansi(line));
        }
    }
    for (path, hits) in grouped {
        if !path.is_empty() {
            body.push(format!(
                "\x1b[36m{}\x1b[0m",
                crate::render::ansi_clip::strip_ansi(path)
            ));
        }
        for (number, content) in hits {
            let line = format!("  {number}: {}", highlight(content, matcher.as_ref()));
            let width = crate::render::fold_text::terminal_wrap_width()
                .saturating_sub(2)
                .max(1);
            for line in crate::render::fold_text::fold_preview_lines(
                &super::result_rows::wrap(&line, width),
                false,
            ) {
                body.push(match line {
                    crate::render::fold_text::FoldedDisplayLine::Line(line) => line,
                    crate::render::fold_text::FoldedDisplayLine::Omitted { omitted, .. } => {
                        crate::render::omitted_line::render_omitted_line(omitted, true)
                    }
                });
            }
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
    // 3. 【终端】【分组边界】命中和长正文已各自折叠，禁止再次折叠文件分组头
    Some(crate::render::render_expand::with_force_expand(|| {
        result_card::render(
            view,
            mode,
            &title,
            if success {
                ToolHealth::Ok
            } else {
                ToolHealth::Err
            },
            &body.join("\n"),
        )
    }))
}

/// 【终端】【搜索高亮】使用搜索正则强调命中片段，非法表达式与零宽匹配保持原文。
/// 参数: `content` 为命中正文，`matcher` 为可选正则；返回: 安全的 ANSI 正文。
fn highlight(content: &str, matcher: Option<&regex::Regex>) -> String {
    let content = super::safe_text::plain(content);
    let Some(matcher) = matcher else {
        return content;
    };
    let mut output = String::new();
    let mut start = 0;
    for matched in matcher
        .find_iter(&content)
        .filter(|matched| !matched.is_empty())
    {
        output.push_str(&content[start..matched.start()]);
        output.push_str("\x1b[1;33m");
        output.push_str(matched.as_str());
        output.push_str("\x1b[0m");
        start = matched.end();
    }
    output.push_str(&content[start..]);
    output
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

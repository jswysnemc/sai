use super::{result_card, ToolView};
use crate::render::{
    status_style::ToolHealth,
    tool_event_line::{tool_event_label_tense, ToolVerbTense},
    ToolCallDisplayMode,
};
use serde_json::Value;

/// 【终端】【浏览器结果】展示操作目标和可阅读内容，屏蔽供模型定位和下一步调用的说明。
/// 参数: 工具视图和模式；返回: 浏览器结果卡片。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "browser" {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let args: Value = serde_json::from_str(&view.arguments).unwrap_or_default();
    let action = args["action"].as_str().unwrap_or_default();
    let mut title =
        tool_event_label_tense("browser", Some(&view.arguments), ToolVerbTense::Perfect);
    let mut body = Vec::new();
    if action == "screenshot"
        || outcome
            .output
            .starts_with("[Image: source: browser screenshot")
    {
        let metadata = outcome.output.lines().next().unwrap_or_default();
        let source = metadata
            .strip_prefix("[Image: source: ")
            .and_then(|text| text.rsplit_once(", image/").map(|(source, _)| source));
        let dimensions = metadata
            .split_whitespace()
            .filter_map(|part| {
                let part = part.trim_end_matches([']', '.', ',']);
                let (width, height) = part.split_once('x')?;
                (!width.is_empty()
                    && !height.is_empty()
                    && width.bytes().all(|c| c.is_ascii_digit())
                    && height.bytes().all(|c| c.is_ascii_digit()))
                .then_some(part)
            })
            .next_back();
        if let Some(reference) = args["ref"].as_str() {
            title.push_str(&format!(" · {reference}"));
        }
        if let Some(dimensions) = dimensions {
            title.push_str(&format!(" · {dimensions}"));
        }
        if let Some(source) = source {
            if let Some((_, url)) = source.rsplit_once(" of ") {
                body.push(url.to_string());
            }
        }
    } else if action == "evaluate" {
        body.push(result_card::markdown(&outcome.output));
    } else {
        for line in outcome.output.lines() {
            if line.starts_with("Call browser with ")
                || line.starts_with("Scroll:")
                || line.starts_with("Refs:")
            {
                continue;
            }
            if let Some(page) = line.strip_prefix("Page: ").filter(|page| !page.is_empty()) {
                title.push_str(&format!(" · {}", result_card::short(page, 48)));
            } else if line.starts_with("Note: page is still loading") {
                body.push("Page is still loading.".into());
            } else if line.starts_with("[snapshot truncated:") {
                body.push("Page content truncated.".into());
            } else if !line.trim().is_empty() {
                body.push(line.strip_prefix("URL: ").unwrap_or(line).to_string());
            }
        }
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        ToolHealth::Ok,
        &body.join("\n"),
    ))
}

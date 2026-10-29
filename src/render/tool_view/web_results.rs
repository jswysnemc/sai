use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

#[derive(Default)]
struct SearchItem {
    title: String,
    url: String,
    snippet: String,
}

/// 【终端】【网页结果】网页搜索只显示可阅读条目，解析并移除搜索引擎的跳转包装。
/// 参数: 工具视图和模式；返回: 搜索结果卡片。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "web_search" {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let mut items: Vec<SearchItem> = Vec::new();
    let mut provider = None;
    let mut source_query = None;
    for line in outcome.output.lines() {
        let line = line.trim();
        if let Some(query) = line.strip_prefix("## Search results for:") {
            source_query = Some(query.trim());
        } else if let Some(name) = line.strip_prefix("**Provider**:") {
            provider = Some(name.trim());
        } else if let Some(title) = line.strip_prefix("### ") {
            let title = title
                .split_once(". ")
                .filter(|(n, _)| n.chars().all(|ch| ch.is_ascii_digit()))
                .map_or(title, |(_, text)| text);
            items.push(SearchItem {
                title: title.into(),
                ..Default::default()
            });
        } else if let Some(item) = items.last_mut() {
            if let Some(url) = line.strip_prefix("**URL**:") {
                item.url = direct_url(url.trim());
            } else if let Some(snippet) = line.strip_prefix("**Snippet**:") {
                item.snippet = snippet.trim().into();
            } else if !line.is_empty() && !item.snippet.is_empty() {
                item.snippet.push(' ');
                item.snippet.push_str(line);
            }
        }
    }
    if provider.is_none() && items.is_empty() {
        return None;
    }
    let args: Value = serde_json::from_str(&view.arguments).unwrap_or_default();
    let query = args["query"].as_str().or(source_query).unwrap_or("web");
    let mut title = format!(
        "Searched the web · {} {} · {}",
        items.len(),
        if items.len() == 1 {
            "result"
        } else {
            "results"
        },
        result_card::short(query, 56)
    );
    if let Some(provider) = provider.filter(|provider| !provider.is_empty()) {
        title.push_str(&format!(" · {provider}"));
    }
    let body = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let mut lines = vec![format!(
                "{}. {}",
                index + 1,
                result_card::markdown(&item.title)
            )];
            if !item.url.is_empty() {
                lines.push(format!("   \x1b[36m{}\x1b[0m", item.url));
            }
            if !item.snippet.is_empty() {
                lines.push(format!("   {}", result_card::markdown(&item.snippet)));
            }
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(result_card::render(
        view,
        mode,
        &title,
        ToolHealth::Ok,
        &body,
    ))
}

/// 【终端】【直达链接】只解包已知搜索引擎跳转，其他链接保持原地址，不发起请求。
/// 参数: 搜索结果地址；返回: 安全的 HTTP/HTTPS 直达地址。
fn direct_url(raw: &str) -> String {
    let normalized = if raw.starts_with("//") {
        format!("https:{raw}")
    } else {
        raw.into()
    };
    let Ok(url) = reqwest::Url::parse(&normalized) else {
        return result_card::short(raw, 512);
    };
    if matches!(
        url.host_str(),
        Some("duckduckgo.com" | "www.duckduckgo.com")
    ) && url.path().starts_with("/l/")
    {
        if let Some((_, target)) = url.query_pairs().find(|(key, _)| key == "uddg") {
            if let Ok(target) = reqwest::Url::parse(&target) {
                if matches!(target.scheme(), "http" | "https") {
                    return target.to_string();
                }
            }
        }
    }
    normalized
}

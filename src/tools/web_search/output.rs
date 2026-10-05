use serde_json::Value;

/// 【Web 搜索】【结果格式化】将供应商通用结果转换为 Markdown。
///
/// 参数:
/// - `query`: 原始搜索关键词
/// - `provider`: 供应商显示名称
/// - `results`: 供应商结果对象列表
///
/// 返回:
/// - Markdown 格式搜索结果
pub(super) fn format_search_results(query: &str, provider: &str, results: Vec<Value>) -> String {
    let mut lines = vec![
        format!("## Search results for: {query}"),
        format!("**Provider**: {provider}\n"),
    ];
    for (index, item) in results.into_iter().enumerate() {
        let title = item
            .get("title")
            .or_else(|| item.pointer("/metadata/title"))
            .and_then(Value::as_str)
            .unwrap_or("Untitled");
        let url = item
            .get("url")
            .or_else(|| item.pointer("/metadata/sourceURL"))
            .or_else(|| item.pointer("/metadata/url"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let snippet = item
            .get("content")
            .or_else(|| item.get("snippet"))
            .or_else(|| item.get("description"))
            .or_else(|| item.get("text"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let raw = item
            .get("raw_content")
            .or_else(|| item.get("markdown"))
            .and_then(Value::as_str)
            .unwrap_or("");
        lines.push(format!("### {}. {title}", index + 1));
        if !url.is_empty() {
            lines.push(format!("**URL**: {url}"));
        }
        if !snippet.is_empty() {
            lines.push(format!("**Snippet**: {}", clip(snippet, 500)));
        }
        if !raw.is_empty() {
            lines.push(format!("**Content**: {}", clip(raw, 800)));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

/// 【Web 搜索】【结果裁剪】按字符数量裁剪单段搜索内容。
///
/// 参数:
/// - `value`: 原始内容
/// - `max_chars`: 最大字符数量
///
/// 返回:
/// - 未超限原文或带省略标记的裁剪文本
fn clip(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!("{}...", value.chars().take(max_chars).collect::<String>())
    }
}

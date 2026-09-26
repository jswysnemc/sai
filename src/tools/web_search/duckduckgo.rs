/// 【Web 搜索】【结果解析】解析 DuckDuckGo HTML 搜索结果。
///
/// 参数:
/// - `html`: DuckDuckGo HTML 响应
/// - `max_results`: 最多保留结果数
///
/// 返回:
/// - 标题、地址和摘要组成的结果列表
pub(super) fn parse_duckduckgo_html(
    html: &str,
    max_results: usize,
) -> Vec<(String, String, String)> {
    let mut results = Vec::new();
    let mut rest = html;
    while let Some(link_pos) = rest.find("result__a") {
        rest = &rest[link_pos..];
        let Some(href_pos) = rest.find("href=\"") else {
            break;
        };
        let href_start = href_pos + "href=\"".len();
        let Some(href_end) = rest[href_start..].find('"') else {
            break;
        };
        let raw_url = html_unescape(&rest[href_start..href_start + href_end]);
        let Some(tag_end) = rest[href_start + href_end..].find('>') else {
            break;
        };
        let title_start = href_start + href_end + tag_end + 1;
        let Some(title_end) = rest[title_start..].find("</a>") else {
            break;
        };
        let title = clean_html_text(&rest[title_start..title_start + title_end]);
        let snippet =
            if let Some(snippet_pos) = rest[title_start + title_end..].find("result__snippet") {
                let snippet_rest = &rest[title_start + title_end + snippet_pos..];
                if let Some(open_end) = snippet_rest.find('>') {
                    if let Some(close) = snippet_rest[open_end + 1..].find("</") {
                        clean_html_text(&snippet_rest[open_end + 1..open_end + 1 + close])
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
        if !title.is_empty() && !raw_url.is_empty() {
            results.push((title, raw_url, snippet));
        }
        if results.len() >= max_results {
            break;
        }
        rest = &rest[title_start + title_end..];
    }
    results
}

/// 【Web 搜索】【结果清理】将 HTML 片段转换为紧凑纯文本。
///
/// 参数:
/// - `value`: HTML 片段
///
/// 返回:
/// - 解码实体并合并空白后的文本
fn clean_html_text(value: &str) -> String {
    html_unescape(&html2text::from_read(value.as_bytes(), 120))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 【Web 搜索】【结果清理】解码搜索结果中常见的 HTML 实体。
///
/// 参数:
/// - `value`: 包含 HTML 实体的文本
///
/// 返回:
/// - 解码后的文本
fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

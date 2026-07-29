use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::env;

/// 【Web 搜索】【TinyFish 请求】使用 TinyFish Search API 执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
/// - `keys`: 配置中的 TinyFish API Key 列表
/// - `base_url`: TinyFish Search API 地址
/// - `location`: 可选国家代码
/// - `language`: 可选语言代码
///
/// 返回:
/// - Markdown 格式的搜索结果
pub(super) async fn search_tinyfish(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
    base_url: &str,
    location: &str,
    language: &str,
) -> Result<String> {
    let Some(key) = first_api_key(keys, "TINYFISH_API_KEY") else {
        bail!("missing TinyFish API key")
    };
    let mut params = vec![("query", query)];
    if !location.is_empty() {
        params.push(("location", location));
    }
    if !language.is_empty() {
        params.push(("language", language));
    }
    let data: Value = client
        .get(base_url.trim())
        .header("X-API-Key", key)
        .query(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let results = data
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(max_results)
        .collect::<Vec<_>>();
    if results.is_empty() {
        bail!("TinyFish returned no results")
    }
    Ok(format_search_results(query, "TinyFish", results))
}

/// 【Web 搜索】【凭据解析】读取第一个可用 API Key，配置为空时可回退到环境变量。
///
/// 参数:
/// - `keys`: 配置中的 API Key 列表
/// - `fallback_env`: 回退环境变量名称
///
/// 返回:
/// - 可用 API Key
fn first_api_key(keys: &[String], fallback_env: &str) -> Option<String> {
    keys.iter()
        .map(|key| key.trim())
        .filter_map(|key| {
            if let Some(env_name) = key.strip_prefix("$env:") {
                env::var(env_name.trim()).ok()
            } else {
                Some(key.to_string())
            }
        })
        .map(|key| key.trim().to_string())
        .find(|key| !key.is_empty())
        .or_else(|| {
            env::var(fallback_env)
                .ok()
                .map(|key| key.trim().to_string())
        })
        .filter(|key| !key.is_empty())
}

/// 【Web 搜索】【Tavily 请求】使用 Tavily Search API 执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
/// - `keys`: 配置中的 Tavily API Key 列表
/// - `base_url`: Tavily Search API 地址
/// - `search_depth`: 搜索深度
/// - `include_answer`: 是否附带生成答案
/// - `include_raw_content`: 是否附带原始正文
///
/// 返回:
/// - Markdown 格式的搜索结果
pub(super) async fn search_tavily(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
    base_url: &str,
    search_depth: &str,
    include_answer: bool,
    include_raw_content: bool,
) -> Result<String> {
    let Some(key) = first_api_key(keys, "TAVILY_API_KEY") else {
        bail!("missing Tavily API key")
    };
    let raw_content = if include_raw_content {
        Value::String("markdown".to_string())
    } else {
        Value::Bool(false)
    };
    let payload = json!({
        "query": query,
        "max_results": max_results.min(20),
        "search_depth": search_depth,
        "include_answer": include_answer,
        "include_raw_content": raw_content
    });
    let data: Value = client
        .post(base_url.trim())
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(format_search_results(
        query,
        "Tavily",
        data.get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    ))
}

/// 【Web 搜索】【Firecrawl 请求】使用 Firecrawl Search API 执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
/// - `keys`: 配置中的 Firecrawl API Key 列表
/// - `base_url`: Firecrawl Search API 地址
/// - `only_main_content`: 是否仅提取页面主要正文
///
/// 返回:
/// - Markdown 格式的搜索结果
pub(super) async fn search_firecrawl(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
    base_url: &str,
    only_main_content: bool,
) -> Result<String> {
    let Some(key) = first_api_key(keys, "FIRECRAWL_API_KEY") else {
        bail!("missing Firecrawl API key")
    };
    let payload = json!({
        "query": query,
        "limit": max_results.min(20),
        "sources": [{"type":"web"}],
        "scrapeOptions": {
            "formats": [{"type":"markdown"}],
            "onlyMainContent": only_main_content
        }
    });
    let data: Value = client
        .post(base_url.trim())
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let raw = data
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(format_search_results(query, "Firecrawl", raw))
}

/// 【Web 搜索】【AnySearch 请求】使用 AnySearch API 执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
/// - `keys`: 配置中的 AnySearch API Key 列表
/// - `base_url`: AnySearch API 地址
///
/// 返回:
/// - Markdown 格式的搜索结果
pub(super) async fn search_anysearch(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
    base_url: &str,
) -> Result<String> {
    let Some(key) = first_api_key(keys, "ANYSEARCH_API_KEY") else {
        bail!("missing AnySearch API key")
    };
    let payload = json!({"query": query, "max_results": max_results.min(20)});
    let data: Value = client
        .post(base_url.trim())
        .bearer_auth(key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(format_search_results(
        query,
        "AnySearch",
        data.get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    ))
}

/// 【Web 搜索】【SearXNG 请求】使用 SearXNG JSON 接口执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
/// - `base_url`: SearXNG 实例地址
/// - `language`: 搜索语言
/// - `safe_search`: 安全搜索等级
///
/// 返回:
/// - Markdown 格式的搜索结果
pub(super) async fn search_searxng(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    base_url: &str,
    language: &str,
    safe_search: u8,
) -> Result<String> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        bail!("missing SearXNG base URL")
    }
    let url = format!(
        "{base_url}/search?q={}&format=json&language={}&safesearch={safe_search}",
        urlencoding::encode(query),
        urlencoding::encode(language.trim())
    );
    let data: Value = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let results = data
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(max_results)
        .collect::<Vec<_>>();
    if results.is_empty() {
        bail!("SearXNG returned no results")
    }
    Ok(format_search_results(query, "SearXNG", results))
}

/// 【Web 搜索】【DuckDuckGo 请求】使用内置 DuckDuckGo HTML 回退执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
///
/// 返回:
/// - Markdown 格式的搜索结果
pub(super) async fn search_duckduckgo(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
) -> Result<String> {
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    let html = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let results = parse_duckduckgo_html(&html, max_results);
    if results.is_empty() {
        bail!("DuckDuckGo returned no parseable results");
    }
    let mut lines = vec![
        format!("## Search results for: {query}"),
        "**Provider**: DuckDuckGo HTML fallback\n".to_string(),
    ];
    for (index, (title, url, snippet)) in results.into_iter().enumerate() {
        lines.push(format!("### {}. {title}", index + 1));
        lines.push(format!("**URL**: {url}"));
        if !snippet.is_empty() {
            lines.push(format!("**Snippet**: {snippet}"));
        }
        lines.push(String::new());
    }
    Ok(lines.join("\n"))
}

/// 【Web 搜索】【结果解析】解析 DuckDuckGo HTML 搜索结果。
///
/// 参数:
/// - `html`: DuckDuckGo HTML 响应
/// - `max_results`: 最多保留结果数
///
/// 返回:
/// - 标题、地址和摘要组成的结果列表
fn parse_duckduckgo_html(html: &str, max_results: usize) -> Vec<(String, String, String)> {
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

/// 【Web 搜索】【结果格式化】将供应商通用结果转换为 Markdown。
///
/// 参数:
/// - `query`: 原始搜索关键词
/// - `provider`: 供应商显示名称
/// - `results`: 供应商结果对象列表
///
/// 返回:
/// - Markdown 格式搜索结果
fn format_search_results(query: &str, provider: &str, results: Vec<Value>) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_api_key_prefers_configured_key() {
        let keys = vec![" configured-key ".to_string()];

        assert_eq!(
            first_api_key(&keys, "SAI_TINYFISH_UNUSED_KEY").as_deref(),
            Some("configured-key")
        );
    }

    #[test]
    fn first_api_key_reads_env_reference() {
        std::env::set_var("SAI_TINYFISH_ENV_REF_KEY", " env-ref-key ");
        let keys = vec!["$env:SAI_TINYFISH_ENV_REF_KEY".to_string()];

        assert_eq!(
            first_api_key(&keys, "SAI_TINYFISH_UNUSED_KEY").as_deref(),
            Some("env-ref-key")
        );
    }

    #[test]
    fn first_api_key_falls_back_to_env() {
        std::env::set_var("SAI_TINYFISH_FALLBACK_KEY", " fallback-key ");

        assert_eq!(
            first_api_key(&[], "SAI_TINYFISH_FALLBACK_KEY").as_deref(),
            Some("fallback-key")
        );
    }
}

use super::{ToolRegistry, ToolSpec};
use crate::config::WebPluginConfig;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{env, time::Duration};

const MAX_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
const DEFAULT_FETCH_MAX_CHARS: usize = 24_000;
const MAX_FETCH_CHARS: usize = 80_000;

pub fn register(registry: &mut ToolRegistry, config: WebPluginConfig) {
    register_search_tool(registry, "web_search", config.clone());
}

pub fn register_fetch(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "web_fetch",
        "Fetch a URL and return markdown, text, or html. Prefer this for opening a known URL. Does not search the web.",
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Fully-qualified http or https URL." },
                "format": { "type": "string", "enum": ["markdown", "text", "html"], "description": "Output format. Defaults to markdown." },
                "timeout": { "type": "integer", "description": "Timeout seconds, max 120." },
                "max_chars": { "type": "integer", "description": "Maximum characters to return. Defaults to 24000, max 80000." }
            },
            "required": ["url"],
            "additionalProperties": false
        }),
        |args| async move { web_fetch(args).await },
    ));
}

fn register_search_tool(registry: &mut ToolRegistry, name: &'static str, config: WebPluginConfig) {
    registry.register(ToolSpec::new(
        name,
        "Search the web. Prefer configured TinyFish, Tavily, Firecrawl, or AnySearch API keys; fallback to SearXNG, then built-in DuckDuckGo HTML search when providers fail.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query." },
                "max_results": { "type": "integer", "description": "Maximum results, default 5." },
                "provider": { "type": "string", "enum": ["auto", "tinyfish", "tavily", "firecrawl", "anysearch", "searxng", "script"], "description": "Search provider." },
                "location": { "type": "string", "description": "Optional country code for TinyFish geo-targeted results, such as US or GB." },
                "language": { "type": "string", "description": "Optional language code for TinyFish result language, such as en or fr." }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        move |args| {
            let config = config.clone();
            async move { web_search(args, config).await }
        },
    ));
}

async fn web_search(args: Value, config: WebPluginConfig) -> Result<String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if query.is_empty() {
        bail!("query is required");
    }
    let max_results = args
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .clamp(1, 10) as usize;
    let provider = args
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("auto");
    let location = args
        .get("location")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let language = args
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    let order: Vec<&str> = if provider == "auto" {
        vec![
            "tinyfish",
            "tavily",
            "firecrawl",
            "anysearch",
            "searxng",
            "script",
        ]
    } else {
        vec![provider]
    };
    for item in order {
        let result = match item {
            "tinyfish" => {
                search_tinyfish(
                    &client,
                    query,
                    max_results,
                    &config.tinyfish_api_keys,
                    &location,
                    &language,
                )
                .await
            }
            "tavily" => search_tavily(&client, query, max_results, &config.tavily_api_keys).await,
            "firecrawl" => {
                search_firecrawl(&client, query, max_results, &config.firecrawl_api_keys).await
            }
            "anysearch" => {
                search_anysearch(&client, query, max_results, &config.anysearch_api_keys).await
            }
            "searxng" => {
                search_searxng(&client, query, max_results, &config.searxng_base_url).await
            }
            "script" => search_duckduckgo(&client, query, max_results).await,
            _ => continue,
        };
        if let Ok(output) = result {
            if !output.trim().is_empty() {
                return Ok(output);
            }
        }
    }
    bail!("no web search provider succeeded; API keys missing/failed, SearXNG unavailable, and built-in DuckDuckGo fallback returned no results")
}

/// 使用 TinyFish Search API 执行网页搜索。
///
/// 参数:
/// - `client`: 复用的 HTTP 客户端
/// - `query`: 搜索关键词
/// - `max_results`: 最多返回结果数
/// - `keys`: 配置中的 TinyFish API Key 列表
/// - `location`: 可选国家代码
/// - `language`: 可选语言代码
///
/// 返回:
/// - Markdown 格式的搜索结果
async fn search_tinyfish(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
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
        .get("https://api.search.tinyfish.ai")
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

/// 读取第一个可用 API Key，配置为空时可回退到环境变量。
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

async fn search_tavily(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
) -> Result<String> {
    let Some(key) = keys.iter().find(|key| !key.trim().is_empty()) else {
        bail!("missing Tavily API key")
    };
    let payload = json!({"query": query, "max_results": max_results.min(20), "search_depth": "basic", "include_answer": false, "include_raw_content": "markdown"});
    let data: Value = client
        .post("https://api.tavily.com/search")
        .bearer_auth(key.trim())
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

async fn search_firecrawl(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
) -> Result<String> {
    let Some(key) = keys.iter().find(|key| !key.trim().is_empty()) else {
        bail!("missing Firecrawl API key")
    };
    let payload = json!({"query": query, "limit": max_results.min(20), "sources": [{"type":"web"}], "scrapeOptions": {"formats": [{"type":"markdown"}], "onlyMainContent": true}});
    let data: Value = client
        .post("https://api.firecrawl.dev/v2/search")
        .bearer_auth(key.trim())
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

async fn search_anysearch(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    keys: &[String],
) -> Result<String> {
    let Some(key) = keys.iter().find(|key| !key.trim().is_empty()) else {
        bail!("missing AnySearch API key")
    };
    let payload = json!({"query": query, "max_results": max_results.min(20)});
    let data: Value = client
        .post("https://api.anysearch.com/v1/search")
        .bearer_auth(key.trim())
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

async fn search_searxng(
    client: &reqwest::Client,
    query: &str,
    max_results: usize,
    base_url: &str,
) -> Result<String> {
    let base_url = base_url.trim().trim_end_matches('/');
    if base_url.is_empty() {
        bail!("missing SearXNG base URL")
    }
    let url = format!(
        "{base_url}/search?q={}&format=json&language=auto&safesearch=0",
        urlencoding::encode(query)
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

async fn search_duckduckgo(
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

fn clean_html_text(value: &str) -> String {
    html_unescape(&html2text::from_read(value.as_bytes(), 120))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

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

fn clip(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!("{}...", value.chars().take(max_chars).collect::<String>())
    }
}

async fn web_fetch(args: Value) -> Result<String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("URL must start with http:// or https://");
    }
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");
    let timeout = args
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .min(120);
    let max_chars = args
        .get("max_chars")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, MAX_FETCH_CHARS as u64) as usize)
        .unwrap_or(DEFAULT_FETCH_MAX_CHARS);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()?;
    let accept = match format {
        "text" => "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
        "html" => "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, */*;q=0.1",
        _ => "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
    };
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36")
        .header("Accept", accept)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?
        .error_for_status()?;
    if response.content_length().unwrap_or(0) > MAX_RESPONSE_SIZE as u64 {
        bail!("response too large (exceeds 5MB limit)");
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_RESPONSE_SIZE {
        bail!("response too large (exceeds 5MB limit)");
    }
    let content = String::from_utf8_lossy(&bytes).to_string();
    let output = if content_type.contains("text/html") {
        match format {
            "html" => content,
            "text" => html2text::from_read(content.as_bytes(), 120),
            _ => html2md::parse_html(&content),
        }
    } else {
        content
    };
    Ok(clip_fetch_output(&output, max_chars))
}

fn clip_fetch_output(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    if total <= max_chars {
        return value.to_string();
    }
    let clipped = value.chars().take(max_chars).collect::<String>();
    format!("{clipped}\n\n[content truncated from {total} chars to {max_chars} chars]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_fetch_output_with_notice() {
        let output = clip_fetch_output("abcdef", 3);

        assert_eq!(output, "abc\n\n[content truncated from 6 chars to 3 chars]");
    }

    #[test]
    fn keeps_short_fetch_output_unchanged() {
        assert_eq!(clip_fetch_output("abc", 3), "abc");
    }

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

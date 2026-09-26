use super::{
    duckduckgo::parse_duckduckgo_html, output::format_search_results, request::SearchInput,
};
use crate::config::WebSearchConfig;
use anyhow::{bail, Context, Result};
use reqwest::{Client, RequestBuilder};
use serde_json::{json, Value};

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

/// 【网页搜索】【供应商请求】接收客户端、供应商、查询和配置；返回沿用原版协议的请求。
pub(super) fn build_request(
    client: &Client,
    provider: &str,
    input: &SearchInput,
    config: &WebSearchConfig,
) -> Result<RequestBuilder> {
    let query = &input.query;
    Ok(match provider {
        "tinyfish" => {
            let key = first_api_key(&config.tinyfish_api_keys, "TINYFISH_API_KEY")?;
            let mut params = vec![("query", query.as_str())];
            if !input.location.is_empty() { params.push(("location", &input.location)); }
            if !input.language.is_empty() { params.push(("language", &input.language)); }
            client.get(config.tinyfish_base_url.trim()).header("X-API-Key", key).query(&params)
        }
        "tavily" => {
            let raw = if config.tavily_include_raw_content { json!("markdown") } else { json!(false) };
            client.post(config.tavily_base_url.trim())
                .bearer_auth(first_api_key(&config.tavily_api_keys, "TAVILY_API_KEY")?)
                .json(&json!({"query":query, "max_results":input.max_results,
                    "search_depth":config.tavily_search_depth, "include_answer":config.tavily_include_answer,
                    "include_raw_content":raw}))
        }
        "firecrawl" => client.post(config.firecrawl_base_url.trim())
            .bearer_auth(first_api_key(&config.firecrawl_api_keys, "FIRECRAWL_API_KEY")?)
            .json(&json!({"query":query,"limit":input.max_results,"sources":[{"type":"web"}],
                "scrapeOptions":{"formats":[{"type":"markdown"}],"onlyMainContent":config.firecrawl_only_main_content}})),
        "anysearch" => client.post(config.anysearch_base_url.trim())
            .bearer_auth(first_api_key(&config.anysearch_api_keys, "ANYSEARCH_API_KEY")?)
            .json(&json!({"query":query,"max_results":input.max_results})),
        "searxng" => {
            let base = config.searxng_base_url.trim().trim_end_matches('/');
            client.get(format!("{base}/search?q={}&format=json&language={}&safesearch={}",
                urlencoding::encode(query), urlencoding::encode(config.searxng_language.trim()), config.searxng_safe_search))
                .header("Accept", "application/json")
        }
        "duckduckgo" => client.get(format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query)))
            .header("User-Agent", USER_AGENT),
        _ => bail!("unknown web search provider"),
    })
}

/// 【网页搜索】【凭据解析】接收配置密钥和回退环境变量名称；返回首个非空密钥。
pub(super) fn first_api_key(keys: &[String], fallback: &str) -> Result<String> {
    keys.iter()
        .filter_map(|key| {
            let key = key.trim();
            if let Some(name) = key.strip_prefix("$env:") {
                std::env::var(name.trim()).ok()
            } else {
                Some(key.to_string())
            }
        })
        .chain(std::env::var(fallback).ok())
        .map(|key| key.trim().to_string())
        .find(|key| !key.is_empty())
        .context("missing search provider API key")
}

/// 【网页搜索】【结果转换】接收供应商、查询和响应文本；返回与历史实现一致的 Markdown。
pub(super) fn render_response(provider: &str, input: &SearchInput, body: &str) -> Result<String> {
    if provider == "duckduckgo" {
        let results = parse_duckduckgo_html(body, input.max_results);
        if results.is_empty() {
            bail!("DuckDuckGo returned no parseable results");
        }
        let mut lines = vec![
            format!("## Search results for: {}", input.query),
            "**Provider**: DuckDuckGo HTML fallback\n".into(),
        ];
        for (index, (title, url, snippet)) in results.into_iter().enumerate() {
            lines.push(format!("### {}. {title}", index + 1));
            lines.push(format!("**URL**: {url}"));
            if !snippet.is_empty() {
                lines.push(format!("**Snippet**: {snippet}"));
            }
            lines.push(String::new());
        }
        return Ok(lines.join("\n"));
    }
    let data: Value = serde_json::from_str(body)?;
    let (label, field, limit) = match provider {
        "tinyfish" => ("TinyFish", "results", true),
        "tavily" => ("Tavily", "results", false),
        "firecrawl" => ("Firecrawl", "data", false),
        "anysearch" => ("AnySearch", "results", false),
        "searxng" => ("SearXNG", "results", true),
        _ => bail!("unknown web search provider"),
    };
    let mut results = data
        .get(field)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if limit {
        results.truncate(input.max_results);
        if results.is_empty() {
            bail!("{label} returned no results");
        }
    }
    Ok(format_search_results(&input.query, label, results))
}

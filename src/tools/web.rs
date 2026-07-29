mod fetch;
mod providers;

use super::{ToolRegistry, ToolSpec};
use crate::config::WebSearchConfig;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

use fetch::web_fetch;
use providers::{
    search_anysearch, search_duckduckgo, search_firecrawl, search_searxng, search_tavily,
    search_tinyfish,
};

/// 【Web 搜索】【工具注册】注册 Web 搜索工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: Web 搜索与供应商配置
///
/// 返回:
/// - 无返回值
pub fn register(registry: &mut ToolRegistry, config: WebSearchConfig) {
    register_search_tool(registry, "web_search", config.clone());
}

/// 【网页读取】【工具注册】注册已知地址网页读取工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无返回值
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

/// 【Web 搜索】【工具注册】按指定名称注册 Web 搜索工具定义。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `name`: 工具名称
/// - `config`: Web 搜索与供应商配置
///
/// 返回:
/// - 无返回值
fn register_search_tool(registry: &mut ToolRegistry, name: &'static str, config: WebSearchConfig) {
    registry.register(ToolSpec::new(
        name,
        "Search the web with the configured provider. Auto mode tries enabled providers in order and uses DuckDuckGo HTML as the final built-in fallback.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query." },
                "max_results": { "type": "integer", "description": "Maximum results. Uses the configured default when omitted." },
                "provider": { "type": "string", "enum": ["auto", "tinyfish", "tavily", "firecrawl", "anysearch", "searxng", "duckduckgo"], "description": "Search provider. Uses the configured default when omitted." },
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

/// 【Web 搜索】【请求路由】根据工具参数和供应商配置执行 Web 搜索。
///
/// 参数:
/// - `args`: 查询词、结果数量、供应商和可选地域参数
/// - `config`: Web 搜索与供应商配置
///
/// 返回:
/// - 首个成功供应商返回的 Markdown 搜索结果
async fn web_search(args: Value, config: WebSearchConfig) -> Result<String> {
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
        .unwrap_or(config.max_results as u64)
        .clamp(1, 10) as usize;
    let provider = args
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or(&config.default_provider);
    let location = args
        .get("location")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&config.tinyfish_default_location)
        .trim()
        .to_string();
    let language = args
        .get("language")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&config.tinyfish_default_language)
        .trim()
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .build()?;
    let order = provider_order(&config, provider);
    if order.is_empty() {
        bail!("web search provider is disabled or unknown: {provider}");
    }
    for item in order {
        let result = match item {
            "tinyfish" => {
                search_tinyfish(
                    &client,
                    query,
                    max_results,
                    &config.tinyfish_api_keys,
                    &config.tinyfish_base_url,
                    &location,
                    &language,
                )
                .await
            }
            "tavily" => {
                search_tavily(
                    &client,
                    query,
                    max_results,
                    &config.tavily_api_keys,
                    &config.tavily_base_url,
                    &config.tavily_search_depth,
                    config.tavily_include_answer,
                    config.tavily_include_raw_content,
                )
                .await
            }
            "firecrawl" => {
                search_firecrawl(
                    &client,
                    query,
                    max_results,
                    &config.firecrawl_api_keys,
                    &config.firecrawl_base_url,
                    config.firecrawl_only_main_content,
                )
                .await
            }
            "anysearch" => {
                search_anysearch(
                    &client,
                    query,
                    max_results,
                    &config.anysearch_api_keys,
                    &config.anysearch_base_url,
                )
                .await
            }
            "searxng" => {
                search_searxng(
                    &client,
                    query,
                    max_results,
                    &config.searxng_base_url,
                    &config.searxng_language,
                    config.searxng_safe_search,
                )
                .await
            }
            "duckduckgo" => search_duckduckgo(&client, query, max_results).await,
            _ => continue,
        };
        if let Ok(output) = result {
            if !output.trim().is_empty() {
                return Ok(output);
            }
        }
    }
    bail!("no enabled web search provider succeeded")
}

/// 【Web 搜索】【供应商路由】生成当前请求允许尝试的供应商顺序。
///
/// 参数:
/// - `config`: Web 搜索配置
/// - `requested`: 请求指定供应商或 auto
///
/// 返回:
/// - 已启用供应商的固定顺序；旧版 script 标识映射到 DuckDuckGo
fn provider_order(config: &WebSearchConfig, requested: &str) -> Vec<&'static str> {
    const AUTO_ORDER: [&str; 6] = [
        "tinyfish",
        "tavily",
        "firecrawl",
        "anysearch",
        "searxng",
        "duckduckgo",
    ];
    let requested = if requested == "script" {
        "duckduckgo"
    } else {
        requested
    };
    if requested == "auto" {
        return AUTO_ORDER
            .into_iter()
            .filter(|provider| config.provider_enabled(provider))
            .collect();
    }
    AUTO_ORDER
        .into_iter()
        .find(|provider| *provider == requested && config.provider_enabled(provider))
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【Web 搜索】【供应商顺序】验证自动模式跳过已停用供应商。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn auto_provider_order_skips_disabled_providers() {
        let mut config = WebSearchConfig::default();
        config.tinyfish_enabled = false;
        config.firecrawl_enabled = false;
        config.anysearch_enabled = false;
        config.searxng_base_url = "https://search.example.test".to_string();

        assert_eq!(
            provider_order(&config, "auto"),
            vec!["tavily", "searxng", "duckduckgo"]
        );
    }

    /// 【Web 搜索】【供应商顺序】验证显式选择已停用供应商时不执行请求。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn explicit_provider_requires_enabled_provider() {
        let mut config = WebSearchConfig::default();
        config.tavily_enabled = false;

        assert!(provider_order(&config, "tavily").is_empty());
        assert_eq!(provider_order(&config, "duckduckgo"), vec!["duckduckgo"]);
        assert_eq!(provider_order(&config, "script"), vec!["duckduckgo"]);
    }
}

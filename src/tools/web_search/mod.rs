mod duckduckgo;
mod http;
mod output;
mod providers;
mod request;

#[cfg(test)]
mod tests;

use super::{ToolRegistry, ToolSpec};
use crate::config::WebSearchConfig;
use anyhow::{bail, Result};
use request::{provider_order, SearchInput};
use serde_json::{json, Value};

/// 【网页搜索】【工具注册】接收工具注册表和搜索配置；按总开关注册原生只读工具。
pub(super) fn register(registry: &mut ToolRegistry, config: &WebSearchConfig) {
    if !config.enabled {
        return;
    }
    let config = config.clone();
    registry.register(ToolSpec::new(
        "web_search",
        "Search the web for current information and source URLs. Auto mode tries enabled providers in order and uses DuckDuckGo HTML as the final built-in fallback.",
        json!({
            "type":"object",
            "properties":{
                "query":{"type":"string","description":"Search query."},
                "max_results":{"type":"integer","description":"Maximum results, normalized to 1–10. Uses the configured default when omitted."},
                "provider":{"type":"string","enum":["auto","tinyfish","tavily","firecrawl","anysearch","searxng","duckduckgo","script"],"description":"Search provider. Uses the configured default when omitted; script aliases duckduckgo."},
                "location":{"type":"string","description":"Optional TinyFish country code, such as US or GB."},
                "language":{"type":"string","description":"Optional TinyFish language code, such as en or zh-CN."}
            },
            "required":["query"],"additionalProperties":false
        }),
        move |args| {
            let config = config.clone();
            async move { search(args, config).await }
        },
    ));
}

/// 【网页搜索】【查询执行】接收工具参数和配置；按顺序返回首个成功结果或统一失败说明。
async fn search(args: Value, mut config: WebSearchConfig) -> Result<String> {
    // 1. 【网页搜索】【输入验证】在构造网络请求前校验配置及查询
    config.normalize_endpoints();
    config.validate()?;
    let input = SearchInput::parse(&args, &config)?;
    let requested = args["provider"]
        .as_str()
        .unwrap_or(&config.default_provider);
    let order = provider_order(&config, requested);
    if order.is_empty() {
        bail!("web search provider is disabled or unknown: {requested}");
    }
    let client = http::client(config.timeout_seconds)?;
    // 2. 【网页搜索】【失败回退】单个供应商缺少密钥、超时或响应无效均继续尝试后续供应商
    for provider in order {
        let result = async {
            let request = providers::build_request(&client, provider, &input, &config)?;
            let body = http::execute(request).await?;
            providers::render_response(provider, &input, &body)
        }
        .await;
        if let Ok(output) = result {
            if !output.trim().is_empty() {
                return Ok(output);
            }
        }
    }
    bail!("no enabled web search provider succeeded")
}

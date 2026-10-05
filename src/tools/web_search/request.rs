use crate::config::WebSearchConfig;
use anyhow::{bail, Result};
use serde_json::Value;

/// 【网页搜索】【查询参数】各供应商共用的归一化输入。
pub(super) struct SearchInput {
    pub query: String,
    pub max_results: usize,
    pub location: String,
    pub language: String,
}

impl SearchInput {
    /// 【网页搜索】【输入准备】接收工具参数和默认配置；返回非空查询与有界结果数量。
    pub(super) fn parse(args: &Value, config: &WebSearchConfig) -> Result<Self> {
        let query = args["query"].as_str().unwrap_or_default().trim();
        if query.is_empty() {
            bail!("query is required");
        }
        Ok(Self {
            query: query.into(),
            max_results: args["max_results"]
                .as_u64()
                .unwrap_or(config.max_results as u64)
                .clamp(1, 10) as usize,
            location: optional_text(&args["location"], &config.tinyfish_default_location),
            language: optional_text(&args["language"], &config.tinyfish_default_language),
        })
    }
}

/// 【网页搜索】【文本回退】接收可选参数与缺省值；返回去除 Unicode 空白的有效文本。
fn optional_text(value: &Value, fallback: &str) -> String {
    value
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .trim()
        .to_string()
}

/// 【网页搜索】【请求顺序】接收配置及供应商选择；返回允许尝试的固定优先级列表。
pub(super) fn provider_order(config: &WebSearchConfig, requested: &str) -> Vec<&'static str> {
    let requested = if requested == "script" {
        "duckduckgo"
    } else {
        requested
    };
    [
        "tinyfish",
        "tavily",
        "firecrawl",
        "anysearch",
        "brave",
        "exa",
        "searxng",
        "duckduckgo",
    ]
    .into_iter()
    .filter(|provider| {
        (requested == "auto" || requested == *provider) && config.provider_enabled(provider)
    })
    .collect()
}

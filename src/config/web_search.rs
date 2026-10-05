use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// 【网页搜索】【配置模型】沿用主配置的 plugins.web 字段及供应商参数。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WebSearchConfig {
    pub enabled: bool,
    pub default_provider: String,
    pub max_results: usize,
    pub timeout_seconds: u64,
    pub tinyfish_enabled: bool,
    pub tinyfish_api_keys: Vec<String>,
    pub tinyfish_base_url: String,
    pub tinyfish_default_location: String,
    pub tinyfish_default_language: String,
    pub tavily_enabled: bool,
    pub tavily_api_keys: Vec<String>,
    pub tavily_base_url: String,
    pub tavily_search_depth: String,
    pub tavily_include_answer: bool,
    pub tavily_include_raw_content: bool,
    pub firecrawl_enabled: bool,
    pub firecrawl_api_keys: Vec<String>,
    pub firecrawl_base_url: String,
    pub firecrawl_only_main_content: bool,
    pub anysearch_enabled: bool,
    pub anysearch_api_keys: Vec<String>,
    pub anysearch_base_url: String,
    pub brave_enabled: bool,
    pub brave_api_keys: Vec<String>,
    pub brave_base_url: String,
    pub exa_enabled: bool,
    pub exa_api_keys: Vec<String>,
    pub exa_base_url: String,
    pub searxng_enabled: bool,
    pub searxng_base_url: String,
    pub searxng_language: String,
    pub searxng_safe_search: u8,
    pub duckduckgo_enabled: bool,
}

impl Default for WebSearchConfig {
    /// 【网页搜索】【默认配置】无参数；返回自动路由、五条结果与二十秒请求超时。
    fn default() -> Self {
        Self {
            enabled: true,
            default_provider: "auto".into(),
            max_results: 5,
            timeout_seconds: 20,
            tinyfish_enabled: true,
            tinyfish_api_keys: Vec::new(),
            tinyfish_base_url: "https://api.search.tinyfish.ai".into(),
            tinyfish_default_location: String::new(),
            tinyfish_default_language: String::new(),
            tavily_enabled: true,
            tavily_api_keys: Vec::new(),
            tavily_base_url: "https://api.tavily.com/search".into(),
            tavily_search_depth: "basic".into(),
            tavily_include_answer: false,
            tavily_include_raw_content: true,
            firecrawl_enabled: true,
            firecrawl_api_keys: Vec::new(),
            firecrawl_base_url: "https://api.firecrawl.dev/v2/search".into(),
            firecrawl_only_main_content: true,
            anysearch_enabled: true,
            anysearch_api_keys: Vec::new(),
            anysearch_base_url: "https://api.anysearch.com/v1/search".into(),
            brave_enabled: true,
            brave_api_keys: Vec::new(),
            brave_base_url: "https://api.search.brave.com/res/v1/web/search".into(),
            exa_enabled: true,
            exa_api_keys: Vec::new(),
            exa_base_url: "https://api.exa.ai/search".into(),
            searxng_enabled: true,
            searxng_base_url: String::new(),
            searxng_language: "auto".into(),
            searxng_safe_search: 0,
            duckduckgo_enabled: true,
        }
    }
}

impl WebSearchConfig {
    /// 【网页搜索】【地址兼容】无参数；原地清理地址并为旧版无协议地址补齐 HTTPS。
    pub(crate) fn normalize_endpoints(&mut self) {
        for endpoint in [
            &mut self.tinyfish_base_url,
            &mut self.tavily_base_url,
            &mut self.firecrawl_base_url,
            &mut self.anysearch_base_url,
            &mut self.brave_base_url,
            &mut self.exa_base_url,
            &mut self.searxng_base_url,
        ] {
            let value = endpoint.trim();
            *endpoint = if value.is_empty() || value.contains("://") {
                value.to_string()
            } else {
                format!("https://{value}")
            };
        }
    }

    /// 【网页搜索】【配置校验】无参数；返回校验结果，错误只包含字段契约。
    pub(crate) fn validate(&self) -> Result<()> {
        if ![
            "auto",
            "tinyfish",
            "tavily",
            "firecrawl",
            "anysearch",
            "brave",
            "exa",
            "searxng",
            "duckduckgo",
            "script",
        ]
        .contains(&self.default_provider.as_str())
        {
            bail!("plugins.web.default_provider is invalid");
        }
        if !(1..=10).contains(&self.max_results) {
            bail!("plugins.web.max_results must be between 1 and 10");
        }
        if !(1..=120).contains(&self.timeout_seconds) {
            bail!("plugins.web.timeout_seconds must be between 1 and 120");
        }
        if !matches!(self.tavily_search_depth.as_str(), "basic" | "advanced") {
            bail!("plugins.web.tavily_search_depth must be basic or advanced");
        }
        if self.searxng_safe_search > 2 {
            bail!("plugins.web.searxng_safe_search must be between 0 and 2");
        }
        for (provider, endpoint) in [
            ("tinyfish", &self.tinyfish_base_url),
            ("tavily", &self.tavily_base_url),
            ("firecrawl", &self.firecrawl_base_url),
            ("anysearch", &self.anysearch_base_url),
            ("brave", &self.brave_base_url),
            ("exa", &self.exa_base_url),
            ("searxng", &self.searxng_base_url),
        ] {
            if endpoint.trim().is_empty() {
                continue;
            }
            let valid = reqwest::Url::parse(endpoint.trim())
                .ok()
                .is_some_and(|url| {
                    matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
                });
            if !valid {
                bail!("plugins.web.{provider}_base_url must be a valid HTTP(S) URL");
            }
        }
        if self.default_provider != "auto" && !self.provider_enabled(&self.default_provider) {
            bail!("plugins.web.default_provider is disabled or missing its endpoint");
        }
        Ok(())
    }

    /// 【网页搜索】【供应商开关】接收供应商标识；返回是否启用且具备必要实例地址。
    pub(crate) fn provider_enabled(&self, provider: &str) -> bool {
        match provider {
            "tinyfish" => self.tinyfish_enabled,
            "tavily" => self.tavily_enabled,
            "firecrawl" => self.firecrawl_enabled,
            "anysearch" => self.anysearch_enabled,
            "brave" => self.brave_enabled,
            "exa" => self.exa_enabled,
            "searxng" => self.searxng_enabled && !self.searxng_base_url.trim().is_empty(),
            "duckduckgo" | "script" => self.duckduckgo_enabled,
            _ => false,
        }
    }
}

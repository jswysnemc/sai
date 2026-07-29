use super::*;

/// 【Web 搜索】【配置兼容】验证旧版搜索配置会补齐供应商详细选项。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn legacy_web_search_config_defaults_provider_options() {
    let config: WebSearchConfig = serde_json::from_value(serde_json::json!({
        "enabled": true,
        "tinyfish_api_keys": [],
        "tavily_api_keys": [],
        "firecrawl_api_keys": [],
        "anysearch_api_keys": [],
        "searxng_base_url": ""
    }))
    .unwrap();

    assert_eq!(config.default_provider, "auto");
    assert_eq!(config.max_results, 5);
    assert_eq!(config.timeout_seconds, 20);
    assert_eq!(config.tavily_search_depth, "basic");
    assert_eq!(config.searxng_safe_search, 0);
    assert!(config.tinyfish_enabled);
    assert!(config.duckduckgo_enabled);
    assert_eq!(config.tavily_base_url, "https://api.tavily.com/search");
}

/// 【Web 搜索】【配置校验】验证搜索供应商参数拒绝无效值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn validate_rejects_invalid_web_search_settings() {
    let mut config = AppConfig::default();
    config.plugins.web.max_results = 0;
    assert!(config.validate().is_err());

    config.plugins.web.max_results = 5;
    config.plugins.web.default_provider = "unknown".to_string();
    assert!(config.validate().is_err());

    config.plugins.web.default_provider = "auto".to_string();
    config.plugins.web.tavily_search_depth = "extreme".to_string();
    assert!(config.validate().is_err());
}

/// 【Web 搜索】【配置兼容】验证旧配置里缺少协议前缀的地址会被补齐而非拒绝加载。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn normalize_endpoints_adds_scheme_to_legacy_urls() {
    let mut config = WebSearchConfig::default();
    config.searxng_base_url = "localhost:8888".to_string();
    config.tavily_base_url = "  https://api.tavily.com/search  ".to_string();

    config.normalize_endpoints();

    assert_eq!(config.searxng_base_url, "https://localhost:8888");
    assert_eq!(config.tavily_base_url, "https://api.tavily.com/search");
    assert!(config.validate().is_ok());
}

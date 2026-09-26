use super::*;
use crate::config::{apply_agent_override, seed_default_agent_profiles, AgentSurface};

/// 【网页搜索测试】【配置兼容】无参数；验证旧配置默认值、地址归一化及保存后重新加载。
#[test]
fn web_search_config_round_trip_preserves_legacy_settings() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web = serde_json::from_value(json!({
        "max_results":9, "tavily_api_keys":["$env:TAVILY_API_KEY"],
        "searxng_base_url":" localhost:8888 ", "default_provider":"script"
    }))
    .unwrap();
    config.save(&paths).unwrap();
    let loaded = AppConfig::load(&paths).unwrap();
    assert_eq!(loaded.plugins.web.max_results, 9);
    assert_eq!(loaded.plugins.web.timeout_seconds, 20);
    assert_eq!(loaded.plugins.web.default_provider, "script");
    assert_eq!(
        loaded.plugins.web.searxng_base_url,
        "https://localhost:8888"
    );
    assert_eq!(loaded.plugins.web.tavily_api_keys, ["$env:TAVILY_API_KEY"]);
    assert!(loaded.plugins.web.duckduckgo_enabled);
}

/// 【网页搜索测试】【配置拒绝】无参数；验证无效参数不进入运行期，错误不会输出字段值。
#[test]
fn web_search_config_rejects_invalid_settings_without_secrets() {
    for patch in [
        json!({"max_results":0}),
        json!({"max_results":11}),
        json!({"timeout_seconds":0}),
        json!({"timeout_seconds":121}),
        json!({"default_provider":"unknown-secret"}),
        json!({"default_provider":"tavily", "tavily_enabled":false}),
        json!({"default_provider":"searxng"}),
        json!({"tavily_search_depth":"secret"}),
        json!({"searxng_safe_search":3}),
        json!({"tinyfish_base_url":"https://[invalid?key=secret"}),
        json!({"tavily_base_url":"file:///secret"}),
    ] {
        let config: WebSearchConfig = serde_json::from_value(patch).unwrap();
        let error = config.validate().unwrap_err().to_string();
        assert!(error.starts_with("plugins.web."));
        assert!(!error.contains("secret"));
    }
    assert!(serde_json::from_value::<WebSearchConfig>(json!({"tavily_api_keys":[false]})).is_err());
}

/// 【网页搜索测试】【预设恢复】无参数；验证缺省与旧版原样白名单恢复搜索，定制白名单保留。
#[test]
fn web_search_presets_upgrade_only_unchanged_legacy_profiles() {
    for profile in seed_default_agent_profiles()
        .into_iter()
        .filter(|profile| profile.id != "cli")
    {
        assert!(profile
            .enabled_tools
            .iter()
            .any(|name| name == "web_search"));
        assert_eq!(
            profile
                .deferred_tools
                .iter()
                .any(|name| name == "web_search"),
            profile.id == crate::config::GATEWAY_AGENT_ID
        );
        let id = profile.id.clone();
        let mut legacy = profile;
        legacy.enabled_tools.retain(|name| name != "web_search");
        let mut config = AppConfig::default();
        config.agents = vec![legacy.clone()];
        let resolved = apply_agent_override(config.clone(), Some(&id), AgentSurface::Web).unwrap();
        assert!(resolved
            .agent_runtime
            .unwrap()
            .enabled_tools
            .iter()
            .any(|name| name == "web_search"));
        config.agents[0].tools_exclusive = true;
        let resolved = apply_agent_override(config.clone(), Some(&id), AgentSurface::Web).unwrap();
        assert!(!resolved
            .agent_runtime
            .unwrap()
            .enabled_tools
            .iter()
            .any(|name| name == "web_search"));
        config.agents[0].tools_exclusive = false;
        config.agents[0].enabled_tools = vec!["read_file".into()];
        let resolved = apply_agent_override(config, Some(&id), AgentSurface::Web).unwrap();
        assert_eq!(resolved.agent_runtime.unwrap().enabled_tools, ["read_file"]);
    }
}

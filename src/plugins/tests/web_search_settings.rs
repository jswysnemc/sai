use super::support::FixtureHost;
use super::web_search_support::{RESULT, TOOL};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::discovery::PluginDescriptor;
use crate::plugins::{self, GrantUpdate};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

/// 【搜索测试】【有效描述】从真实配置读取搜索包及其固定运行时设置。
/// @param config 主配置；paths 为独立测试路径
/// @returns 已发现的搜索包描述
fn discover(config: &AppConfig, paths: &SaiPaths) -> PluginDescriptor {
    let found = plugins::discover(config, paths);
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    found
        .plugins
        .into_iter()
        .find(|item| item.package.manifest.id == "web-search")
        .unwrap()
}

/// 【搜索测试】【入口兼容】普通与计划模式使用同一 Lua 工具，显式插件开关覆盖旧开关。
#[test]
fn both_registries_use_the_lua_search_and_preserve_enable_precedence() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.enabled = false;
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(TOOL));
    for (enabled, legacy) in [(true, false), (false, true)] {
        config.plugins.web.enabled = legacy;
        plugins::set_enabled(&config, &paths, "web-search", enabled, GrantUpdate::Keep).unwrap();
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert_eq!(registry.contains(TOOL), enabled);
            assert!(registry.contains("web_fetch"));
            assert!(registry.plugin_diagnostics().is_empty());
            if enabled {
                let filtered = registry.clone_filtered(&[TOOL]);
                assert_eq!(filtered.definitions().len(), 1);
                assert!(filtered
                    .active_plugins()
                    .contains(&("web-search".into(), "1.0.0".into())));
            }
        }
    }
    assert!(crate::tools::tool_catalog(&config, &paths)
        .iter()
        .any(|tool| tool.name == TOOL));
}

/// 【搜索测试】【凭据保存】环境凭据只进入快照，启停与配置操作不把解析结果写回磁盘。
#[test]
fn resolved_credentials_are_not_persisted_by_plugin_management() {
    const NAME: &str = "SAI_LUA_SEARCH_PERSIST_TEST_KEY";
    std::env::set_var(NAME, " resolved-private-key ");
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.tavily_api_keys = vec!["legacy-private-key".into()];
    config.plugins.web.max_results = 7;
    let settings =
        json!({"max_results":2,"tavily_api_keys":["",format!("$env:{NAME}"),"unused-key"]});
    plugins::configure(&config, &paths, "web-search", settings.clone()).unwrap();
    let descriptor = discover(&config, &paths);
    assert_eq!(
        descriptor.settings()["tavily_api_keys"],
        json!(["resolved-private-key"])
    );
    assert_eq!(descriptor.settings()["max_results"], 2);
    assert!(descriptor.settings().get("providers").is_none());
    assert!(descriptor.settings().get("tools").is_none());
    for enabled in [false, true] {
        plugins::set_enabled(&config, &paths, "web-search", enabled, GrantUpdate::Keep).unwrap();
        assert_eq!(
            load_config(&paths).unwrap().plugins["web-search"].settings,
            settings
        );
        let persisted = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
        assert!(!persisted.contains("resolved-private-key"));
        assert!(!persisted.contains("legacy-private-key"));
    }
    let previous = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(plugins::configure(
        &config,
        &paths,
        "web-search",
        json!({"timeout_seconds":121})
    )
    .is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        previous
    );
    std::env::remove_var(NAME);
}

/// 【搜索测试】【旧配置延续】管理开关不会复制旧配置默认值，后续旧字段修改仍能生效。
#[test]
fn enabling_a_plugin_does_not_pin_legacy_settings() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.max_results = 4;
    plugins::set_enabled(&config, &paths, "web-search", true, GrantUpdate::Keep).unwrap();
    assert_eq!(
        load_config(&paths).unwrap().plugins["web-search"].settings,
        json!({})
    );
    assert_eq!(discover(&config, &paths).settings()["max_results"], 4);
    config.plugins.web.max_results = 8;
    assert_eq!(discover(&config, &paths).settings()["max_results"], 8);
}

/// 【搜索测试】【外部隔离】外部包只取得自己的设置，不能通过兼容层继承搜索凭据。
#[test]
fn external_plugins_cannot_inherit_legacy_search_settings() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.tavily_api_keys = vec!["private-search-key".into()];
    let mut descriptor = super::support::descriptor("external-search", "");
    descriptor.setting.settings = json!({"own_setting":"value"});
    descriptor.refresh_compatibility(&config, &paths).unwrap();
    assert_eq!(descriptor.settings(), &json!({"own_setting":"value"}));
    assert!(!serde_json::to_string(descriptor.settings())
        .unwrap()
        .contains("private-search-key"));
}

/// 【搜索测试】【授权固定】显式授权后更换搜索地址不会自动扩大网络访问范围。
#[tokio::test]
async fn changing_an_endpoint_preserves_explicit_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.tavily_api_keys = vec!["test-key".into()];
    plugins::set_enabled(&config, &paths, "web-search", true, GrantUpdate::Declared).unwrap();
    plugins::configure(
        &config,
        &paths,
        "web-search",
        json!({
            "tavily_base_url":"https://new.example.test/search?token=private-query",
        }),
    )
    .unwrap();
    let descriptor = discover(&config, &paths);
    assert!(descriptor
        .capabilities()
        .http
        .contains("https://new.example.test"));
    assert!(!descriptor
        .grants()
        .http
        .contains("https://new.example.test"));
    assert!(descriptor
        .capabilities()
        .http_read_only_post
        .contains("https://new.example.test/search"));
    assert!(
        !serde_json::to_string(&descriptor.runtime_package().manifest)
            .unwrap()
            .contains("private-query")
    );
    let host = Arc::new(FixtureHost::new(&[]));
    let plugin = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host.clone(),
    )
    .unwrap();
    assert!(plugin
        .call_tool(
            TOOL,
            json!({"query":"Rust","provider":"tavily"}),
            InvocationContext::default()
        )
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【搜索测试】【凭据轮换】新快照识别环境值改变，已经加载的实例继续使用原有凭据。
#[tokio::test]
async fn credential_rotation_changes_new_snapshots_without_mutating_existing_instances() {
    const NAME: &str = "SAI_LUA_SEARCH_ROTATE_TEST_KEY";
    std::env::set_var(NAME, "first-key");
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.web.tavily_api_keys = vec![format!("$env:{NAME}")];
    let first = discover(&config, &paths);
    let host = Arc::new(FixtureHost::new(&[(200, RESULT)]));
    let plugin = PluginRuntime::load(
        first.runtime_package(),
        first.settings().clone(),
        first.grants(),
        host.clone(),
    )
    .unwrap();
    std::env::set_var(NAME, "second-key");
    let second = discover(&config, &paths);
    assert_ne!(first.revision().unwrap(), second.revision().unwrap());
    assert_eq!(second.settings()["tavily_api_keys"], json!(["second-key"]));
    plugin
        .call_tool(
            TOOL,
            json!({"query":"Rust","provider":"tavily"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        host.requests.lock().unwrap()[0].headers["authorization"],
        "Bearer first-key"
    );
    std::env::remove_var(NAME);
}

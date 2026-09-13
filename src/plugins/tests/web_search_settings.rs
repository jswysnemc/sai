use super::support::FixtureHost;
use super::web_search_support::{RESULT, TOOL};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::discovery::find;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

const ID: &str = "web-search";
const PUBLIC: &str = "lua__web-search__web_search";

/// 【搜索测试】【普通安装】创建不继承主配置或凭据的外部搜索包
/// @returns 临时目录、隔离路径和默认主配置
fn fixture() -> (tempfile::TempDir, SaiPaths, AppConfig) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    super::example_support::install(ID, &paths);
    (root, paths, AppConfig::default())
}

/// 【搜索测试】【入口一致】普通与只读入口遵守外部包启用状态和命名空间
/// @returns 无；不再注册短名称或在禁用时留下内置目录项
#[test]
fn both_registries_use_the_installed_search_plugin() {
    let (_root, paths, config) = fixture();
    for enabled in [false, true, false] {
        plugins::set_enabled(&config, &paths, ID, enabled, GrantUpdate::Keep).unwrap();
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(!registry.contains(TOOL));
            assert_eq!(registry.contains(PUBLIC), enabled);
            assert!(registry.plugin_diagnostics().is_empty());
            assert!(registry.contains("read_file"));
            if enabled {
                assert_eq!(
                    registry.clone_filtered(&[PUBLIC]).plugin_owner(PUBLIC),
                    Some(ID)
                );
            }
        }
        assert_eq!(
            crate::tools::tool_catalog(&config, &paths)
                .iter()
                .any(|tool| tool.name == PUBLIC),
            enabled
        );
    }
}

/// 【搜索测试】【独立设置】旧主配置不会提供密钥或默认值，环境引用原样保存
/// @returns 无；初始化和管理操作不读取环境凭据
#[test]
fn search_settings_do_not_inherit_or_resolve_legacy_credentials() {
    let (_root, paths, config) = fixture();
    let mut legacy = serde_json::to_value(config).unwrap();
    legacy["plugins"]["web"] =
        json!({"enabled":true,"tavily_api_keys":["legacy-private-key"],"max_results":9});
    let config: AppConfig = serde_json::from_value(legacy).unwrap();
    let descriptor = find(&config, &paths, ID).unwrap();
    assert_eq!(descriptor.settings(), &json!({}));
    assert!(descriptor.grants().http.is_empty());
    assert!(descriptor.grants().system.environment.is_empty());
    let settings = json!({"max_results":2,"tavily_api_keys":["$env:TAVILY_API_KEY"]});
    plugins::configure(&config, &paths, ID, settings.clone()).unwrap();
    for enabled in [false, true] {
        plugins::set_enabled(&config, &paths, ID, enabled, GrantUpdate::Keep).unwrap();
        assert_eq!(find(&config, &paths, ID).unwrap().settings(), &settings);
        assert_eq!(load_config(&paths).unwrap().plugins[ID].settings, settings);
        let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
        assert!(!saved.contains("legacy-private-key"));
    }
}

/// 【搜索测试】【原子验证】独立设置继续校验数值、供应商、数组和类型
/// @returns 无；非法设置不改写已有文件
#[test]
fn invalid_search_settings_preserve_the_saved_configuration() {
    let (_root, paths, config) = fixture();
    plugins::configure(&config, &paths, ID, json!({"max_results":2})).unwrap();
    let file = paths.config_dir.join("plugins.jsonc");
    let previous = std::fs::read(&file).unwrap();
    for settings in [
        json!({"max_results":0}),
        json!({"timeout_seconds":121}),
        json!({"default_provider":"unknown"}),
        json!({"default_provider":"tavily","tavily_enabled":false}),
        json!({"tavily_search_depth":"extreme"}),
        json!({"searxng_safe_search":3}),
        json!({"tavily_api_keys":[false]}),
        json!({"tavily_api_keys":"secret"}),
    ] {
        assert!(
            plugins::configure(&config, &paths, ID, settings.clone()).is_err(),
            "{settings}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), previous);
    }
}

/// 【搜索测试】【授权交集】更换搜索地址既不扩展清单，也不扩大用户授权
/// @returns 无；仅修改设置或仅申请声明之外的来源都不能发起请求
#[tokio::test]
async fn changing_an_endpoint_cannot_expand_manifest_or_grants() {
    let (_root, paths, config) = fixture();
    plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Declared).unwrap();
    plugins::configure(&config, &paths, ID, json!({"tavily_api_keys":["test-key"],"tavily_base_url":"https://new.example.test/search?token=private-query"})).unwrap();
    let descriptor = find(&config, &paths, ID).unwrap();
    assert!(!descriptor
        .capabilities()
        .http
        .contains("https://new.example.test"));
    assert!(!descriptor
        .grants()
        .http
        .contains("https://new.example.test"));
    assert!(
        !serde_json::to_string(&descriptor.runtime_package().manifest)
            .unwrap()
            .contains("private-query")
    );
    assert!(plugins::set_enabled(
        &config,
        &paths,
        ID,
        true,
        GrantUpdate::Changes(GrantChanges {
            http: Some(["https://new.example.test".into()].into()),
            ..Default::default()
        })
    )
    .is_err());
    let host = Arc::new(FixtureHost::new(&[]));
    let runtime = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host.clone(),
    )
    .unwrap();
    assert!(runtime
        .call_tool(
            TOOL,
            json!({"query":"Rust","provider":"tavily"}),
            InvocationContext::default()
        )
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【搜索测试】【凭据快照】显式配置轮换生成新修订，既有实例继续使用原始设置
/// @returns 无；新设置不原地修改已加载实例
#[tokio::test]
async fn credential_configuration_changes_only_new_snapshots() {
    let (_root, paths, config) = fixture();
    plugins::configure(
        &config,
        &paths,
        ID,
        json!({"tavily_api_keys":["first-key"]}),
    )
    .unwrap();
    plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Declared).unwrap();
    let first = find(&config, &paths, ID).unwrap();
    let host = Arc::new(FixtureHost::new(&[(200, RESULT)]));
    let runtime = PluginRuntime::load(
        first.runtime_package(),
        first.settings().clone(),
        first.grants(),
        host.clone(),
    )
    .unwrap();
    plugins::configure(
        &config,
        &paths,
        ID,
        json!({"tavily_api_keys":["second-key"]}),
    )
    .unwrap();
    let second = find(&config, &paths, ID).unwrap();
    assert_ne!(first.revision().unwrap(), second.revision().unwrap());
    assert_eq!(second.settings()["tavily_api_keys"], json!(["second-key"]));
    runtime
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
}

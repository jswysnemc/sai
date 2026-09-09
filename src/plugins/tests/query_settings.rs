use super::query_support::{discover, QueryHost, QUERIES};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

/// 【查询插件测试】【开关优先级】三个旧开关决定缺省状态，显式插件开关覆盖普通和只读入口。
#[test]
fn query_plugin_switches_override_legacy_defaults_in_both_registries() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.weather.enabled = false;
    config.plugins.exchange_rate.enabled = false;
    config.plugins.moegirl.enabled = false;
    for registry in [
        crate::tools::builtin_registry_without_mcp(&config, &paths),
        crate::tools::readonly_registry(&config, &paths),
    ] {
        for (_, tool) in QUERIES {
            assert!(!registry.contains(tool));
        }
    }
    for (enabled, legacy) in [(true, false), (false, true)] {
        config.plugins.weather.enabled = legacy;
        config.plugins.exchange_rate.enabled = legacy;
        config.plugins.moegirl.enabled = legacy;
        for (id, _) in QUERIES {
            plugins::set_enabled(&config, &paths, id, enabled, GrantUpdate::Keep).unwrap();
        }
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for (id, tool) in QUERIES {
                assert_eq!(registry.contains(tool), enabled, "{id}");
                if enabled {
                    assert_eq!(registry.plugin_owner(tool), Some(id));
                    let filtered = registry.clone_filtered(&[tool]);
                    assert_eq!(filtered.definitions().len(), 1);
                    assert!(filtered
                        .active_plugins()
                        .contains(&(id.into(), "1.0.0".into())));
                }
            }
        }
    }
}

/// 【查询插件测试】【独立启停】禁用单个包只移除自身工具，目录仍保留供 Agent 白名单配置。
#[test]
fn query_plugins_can_be_disabled_independently_without_losing_catalog_entries() {
    for (disabled, _) in QUERIES {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let config = AppConfig::default();
        plugins::set_enabled(&config, &paths, disabled, false, GrantUpdate::Keep).unwrap();
        let catalog = crate::tools::tool_catalog(&config, &paths);
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.contains("web_fetch"));
            for (id, tool) in QUERIES {
                assert_eq!(
                    registry.contains(tool),
                    id != disabled,
                    "{disabled}: {tool}"
                );
                assert!(catalog.iter().any(|entry| entry.name == tool));
            }
        }
    }
}

/// 【查询插件测试】【网络撤权】清空 HTTP 授权后所有查询在到达宿主前失败，重新启用不会恢复授权。
#[tokio::test]
async fn query_http_revocation_blocks_requests_and_survives_reenabling() {
    for (id, tool, args) in [
        ("weather", "get_weather", json!({"location":"Beijing"})),
        (
            "exchange-rate",
            "get_exchange_rate",
            json!({"base":"USD","target":"CNY"}),
        ),
        ("moegirl", "query_moegirl", json!({"title":"页面"})),
    ] {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let config = AppConfig::default();
        assert!(!discover(&config, &paths, id).grants().http.is_empty());
        plugins::set_enabled(
            &config,
            &paths,
            id,
            true,
            GrantUpdate::Changes(GrantChanges {
                http: Some(BTreeSet::new()),
                ..Default::default()
            }),
        )
        .unwrap();
        for enabled in [false, true] {
            plugins::set_enabled(&config, &paths, id, enabled, GrantUpdate::Keep).unwrap();
        }
        let descriptor = discover(&config, &paths, id);
        assert!(descriptor.grants().http.is_empty());
        let host = Arc::new(QueryHost::new(&[]));
        let plugin = PluginRuntime::load(
            descriptor.runtime_package(),
            descriptor.settings().clone(),
            descriptor.grants(),
            host.clone(),
        )
        .unwrap();
        assert!(plugin
            .call_tool(tool, args, InvocationContext::default())
            .await
            .is_err());
        assert!(host.requests.lock().unwrap().is_empty());
    }
}

/// 【查询插件测试】【日文站跳转】缺省授权覆盖旧域名的实际 301 目标，用户显式授权仍保持固定。
#[test]
fn moegirl_defaults_allow_the_japanese_redirect_without_expanding_explicit_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let original = "https://ja.moegirl.org/api.php?action=opensearch&search=test";
    let target = "https://ja.moegirl.org.cn/api.php?action=opensearch&search=test";
    let descriptor = discover(&config, &paths, "moegirl");
    let effective = descriptor.grants().intersection(descriptor.capabilities());
    effective.authorize_request("GET", original, false).unwrap();
    effective.authorize_request("GET", target, false).unwrap();

    plugins::set_enabled(
        &config,
        &paths,
        "moegirl",
        true,
        GrantUpdate::Changes(GrantChanges {
            http: Some(["https://ja.moegirl.org".into()].into()),
            ..Default::default()
        }),
    )
    .unwrap();
    plugins::set_enabled(&config, &paths, "moegirl", true, GrantUpdate::Keep).unwrap();
    let descriptor = discover(&config, &paths, "moegirl");
    let effective = descriptor.grants().intersection(descriptor.capabilities());
    effective.authorize_request("GET", original, false).unwrap();
    assert!(effective.authorize_request("GET", target, false).is_err());
    plugins::set_enabled(&config, &paths, "moegirl", true, GrantUpdate::Declared).unwrap();
    discover(&config, &paths, "moegirl")
        .grants()
        .authorize_request("GET", target, false)
        .unwrap();
}

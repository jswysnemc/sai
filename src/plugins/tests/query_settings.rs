use super::example_support::install;
use super::query_support::{discover, QueryHost};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;

const QUERIES: [(&str, &str); 3] = [
    ("weather", "get_weather"),
    ("exchange-rate", "get_exchange_rate"),
    ("moegirl", "query_moegirl"),
];

/// 【查询插件测试】【独立启停】普通与只读注册入口均使用已安装包的公开名称
/// @returns 无；缺省禁用，显式启用不自动授予网络
#[test]
fn query_plugin_switches_control_installed_names_in_both_registries() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    for (id, _) in QUERIES {
        install(id, &paths);
        assert!(discover(&config, &paths, id).grants().http.is_empty());
    }
    for enabled in [false, true, false] {
        for (id, _) in QUERIES {
            plugins::set_enabled(&config, &paths, id, enabled, GrantUpdate::Keep).unwrap();
        }
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.plugin_diagnostics().is_empty());
            for (id, local) in QUERIES {
                let name = format!("lua__{id}__{local}");
                assert!(!registry.contains(local));
                assert_eq!(registry.contains(&name), enabled, "{id}");
                if enabled {
                    let filtered = registry.clone_filtered(&[&name]);
                    assert_eq!(filtered.definitions().len(), 1);
                    assert_eq!(filtered.plugin_owner(&name), Some(id));
                }
            }
        }
    }
}

/// 【查询插件测试】【包间隔离】禁用一个查询包只移除自身工具和目录项
/// @returns 无；其他已安装查询和核心文件工具保持可用
#[test]
fn query_plugins_can_be_disabled_independently() {
    for (disabled, _) in QUERIES {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let config = AppConfig::default();
        for (id, _) in QUERIES {
            install(id, &paths);
            plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
        }
        plugins::set_enabled(&config, &paths, disabled, false, GrantUpdate::Keep).unwrap();
        let catalog = crate::tools::tool_catalog(&config, &paths);
        for registry in [
            crate::tools::builtin_registry_without_mcp(&config, &paths),
            crate::tools::readonly_registry(&config, &paths),
        ] {
            assert!(registry.contains("read_file"));
            for (id, local) in QUERIES {
                let name = format!("lua__{id}__{local}");
                assert_eq!(
                    registry.contains(&name),
                    id != disabled,
                    "{disabled}: {name}"
                );
                assert_eq!(
                    catalog.iter().any(|entry| entry.name == name),
                    id != disabled
                );
            }
        }
    }
}

/// 【查询插件测试】【网络撤权】撤销显式网络授权后重新启用也不能请求宿主
/// @returns 无；三个独立包均保持用户撤权结果
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
        install(id, &paths);
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Declared).unwrap();
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
        let runtime = PluginRuntime::load(
            descriptor.runtime_package(),
            descriptor.settings().clone(),
            descriptor.grants(),
            host.clone(),
        )
        .unwrap();
        assert!(runtime
            .call_tool(tool, args, InvocationContext::default())
            .await
            .is_err());
        assert!(host.requests.lock().unwrap().is_empty());
    }
}

/// 【查询插件测试】【精确重定向】日文百科跳转来源必须分别取得用户授权
/// @returns 无；清单包含来源不等于用户已经授权
#[test]
fn moegirl_redirects_require_explicit_origin_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    install("moegirl", &paths);
    let original = "https://ja.moegirl.org/api.php?action=opensearch&search=test";
    let target = "https://ja.moegirl.org.cn/api.php?action=opensearch&search=test";
    assert!(discover(&config, &paths, "moegirl")
        .grants()
        .authorize_request("GET", original, false)
        .is_err());
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

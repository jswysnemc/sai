use super::query_support::{discover, QueryHost};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::{self, GrantUpdate};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

const ID: &str = "exchange-rate";

/// 【汇率设置测试】【字段覆盖】只覆盖显式字段，空密钥和 false 都能覆盖旧值。
#[test]
fn exchange_settings_override_legacy_values_per_field() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.exchange_rate.api_key = "legacy-private-key".into();
    config.plugins.exchange_rate.free_fallback_enabled = false;
    assert_eq!(
        discover(&config, &paths, ID).settings(),
        &json!({
            "api_key":"legacy-private-key", "free_fallback_enabled":false,
        })
    );
    for (settings, expected) in [
        (
            json!({"free_fallback_enabled":true}),
            json!({"api_key":"legacy-private-key","free_fallback_enabled":true}),
        ),
        (
            json!({"api_key":""}),
            json!({"api_key":"","free_fallback_enabled":false}),
        ),
        (
            json!({"api_key":"explicit-key","free_fallback_enabled":false}),
            json!({"api_key":"explicit-key","free_fallback_enabled":false}),
        ),
    ] {
        plugins::configure(&config, &paths, ID, settings.clone()).unwrap();
        assert_eq!(discover(&config, &paths, ID).settings(), &expected);
        assert_eq!(load_config(&paths).unwrap().plugins[ID].settings, settings);
    }
}

/// 【汇率设置测试】【凭据保存】配置与启停只保存显式设置，旧密钥和派生默认值不会复制到插件配置。
#[test]
fn exchange_management_does_not_persist_legacy_credentials_or_pin_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.exchange_rate.api_key = "first-legacy-key".into();
    plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Keep).unwrap();
    assert_eq!(load_config(&paths).unwrap().plugins[ID].settings, json!({}));
    assert_eq!(
        discover(&config, &paths, ID).settings()["api_key"],
        "first-legacy-key"
    );
    config.plugins.exchange_rate.api_key = "second-legacy-key".into();
    config.plugins.exchange_rate.free_fallback_enabled = false;
    assert_eq!(
        discover(&config, &paths, ID).settings(),
        &json!({
            "api_key":"second-legacy-key", "free_fallback_enabled":false,
        })
    );
    let settings = json!({"free_fallback_enabled":true});
    plugins::configure(&config, &paths, ID, settings.clone()).unwrap();
    for enabled in [false, true] {
        plugins::set_enabled(&config, &paths, ID, enabled, GrantUpdate::Keep).unwrap();
        assert_eq!(load_config(&paths).unwrap().plugins[ID].settings, settings);
        let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
        assert!(!saved.contains("first-legacy-key"));
        assert!(!saved.contains("second-legacy-key"));
        assert!(!saved.contains("api_key"));
    }
}

/// 【汇率设置测试】【原子验证】非法配置不改写已有文件，合法 false 进入运行时后确实阻止免费请求。
#[tokio::test]
async fn invalid_exchange_settings_preserve_the_file_and_false_disables_fallback() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    plugins::configure(
        &config,
        &paths,
        ID,
        json!({"api_key":"","free_fallback_enabled":false}),
    )
    .unwrap();
    let file = paths.config_dir.join("plugins.jsonc");
    let previous = std::fs::read(&file).unwrap();
    for invalid in [
        json!(null),
        json!([]),
        json!(true),
        json!({"api_key":false}),
        json!({"api_key":42}),
        json!({"api_key":null}),
        json!({"free_fallback_enabled":"false"}),
        json!({"free_fallback_enabled":0}),
        json!({"free_fallback_enabled":null}),
    ] {
        assert!(
            plugins::configure(&config, &paths, ID, invalid.clone()).is_err(),
            "{invalid}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), previous);
    }
    let descriptor = discover(&config, &paths, ID);
    let host = Arc::new(QueryHost::new(&[]));
    let plugin = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host.clone(),
    )
    .unwrap();
    let error = plugin
        .call_tool(
            "get_exchange_rate",
            json!({"base":"USD","target":"CNY"}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("free fallback is disabled"));
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【汇率设置测试】【凭据隔离】外部包即使使用相同标识也不能取得兼容密钥，其他内置包同样隔离。
#[test]
fn legacy_exchange_credentials_are_scoped_to_the_bundled_exchange_package() {
    let mut config = AppConfig::default();
    config.plugins.exchange_rate.api_key = "private-exchange-key".into();
    for id in [ID, "external-exchange"] {
        let mut descriptor = super::support::descriptor(id, "");
        descriptor.setting.settings = json!({"own_setting":"value"});
        descriptor.refresh_compatibility(&config).unwrap();
        assert_eq!(descriptor.settings(), &json!({"own_setting":"value"}));
    }
    let root = tempfile::tempdir().unwrap();
    for descriptor in plugins::discover(&config, &SaiPaths::for_tests(root.path())).plugins {
        if descriptor.package.manifest.id != ID {
            assert!(!serde_json::to_string(descriptor.settings())
                .unwrap()
                .contains("private-exchange-key"));
        }
    }
}

/// 【汇率设置测试】【快照更新】旧设置变更只影响新实例，已加载查询保留固定凭据和修订标识。
#[tokio::test]
async fn exchange_credential_changes_affect_new_snapshots_only() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.exchange_rate.api_key = "first-key".into();
    let first = discover(&config, &paths, ID);
    let revision = first.revision().unwrap();
    let host = Arc::new(QueryHost::new(&[Ok((
        200,
        r#"{"result":"success","conversion_rates":{"CNY":7}}"#,
    ))]));
    let plugin = PluginRuntime::load(
        first.runtime_package(),
        first.settings().clone(),
        first.grants(),
        host.clone(),
    )
    .unwrap();
    config.plugins.exchange_rate.api_key = "second-key".into();
    let second = discover(&config, &paths, ID);
    assert_ne!(second.revision().unwrap(), revision);
    assert_eq!(first.revision().unwrap(), revision);
    assert_eq!(second.settings()["api_key"], "second-key");
    assert_eq!(
        plugin
            .call_tool(
                "get_exchange_rate",
                json!({"base":"USD","target":"CNY"}),
                InvocationContext::default()
            )
            .await
            .unwrap(),
        "USD 到 CNY 的汇率是: 7"
    );
    assert_eq!(
        host.requests.lock().unwrap()[0].url,
        "https://v6.exchangerate-api.com/v6/first-key/latest/USD"
    );
}

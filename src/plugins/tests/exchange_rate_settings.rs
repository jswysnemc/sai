use super::query_support::{discover, QueryHost};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::{self, GrantUpdate};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

const ID: &str = "exchange-rate";

/// 【汇率设置测试】【普通安装】创建不继承主配置的独立示例
/// @returns 临时根目录、应用路径和配置
fn fixture() -> (tempfile::TempDir, SaiPaths, AppConfig) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    super::example_support::install(ID, &paths);
    (root, paths, AppConfig::default())
}

/// 【汇率设置测试】【配置隔离】旧主配置凭据不会进入外部包，显式设置按完整对象保存
/// @returns 无；空密钥和 false 均保留，启停不改变独立设置
#[test]
fn exchange_settings_are_independent_of_legacy_config() {
    let (_root, paths, config) = fixture();
    let mut legacy = serde_json::to_value(config).unwrap();
    legacy["plugins"]["exchange_rate"] =
        json!({"enabled":true,"api_key":"legacy-private-key","free_fallback_enabled":false});
    let config: AppConfig = serde_json::from_value(legacy).unwrap();
    assert_eq!(discover(&config, &paths, ID).settings(), &json!({}));
    for settings in [
        json!({"free_fallback_enabled":true}),
        json!({"api_key":""}),
        json!({"api_key":"explicit-key","free_fallback_enabled":false}),
    ] {
        plugins::configure(&config, &paths, ID, settings.clone()).unwrap();
        for enabled in [false, true] {
            plugins::set_enabled(&config, &paths, ID, enabled, GrantUpdate::Keep).unwrap();
            assert_eq!(discover(&config, &paths, ID).settings(), &settings);
            assert_eq!(load_config(&paths).unwrap().plugins[ID].settings, settings);
        }
        let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
        assert!(!saved.contains("legacy-private-key"));
    }
}

/// 【汇率设置测试】【原子验证】非法配置保留已有文件，关闭回退后不请求免费接口
/// @returns 无；错误发生在业务请求前
#[tokio::test]
async fn invalid_exchange_settings_preserve_the_file_and_false_disables_fallback() {
    let (_root, paths, config) = fixture();
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
    let runtime = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host.clone(),
    )
    .unwrap();
    let error = runtime
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

/// 【汇率设置测试】【快照更新】保存新凭据只影响新实例，已有查询保持原设置
/// @returns 无；实际请求仍使用原快照密钥
#[tokio::test]
async fn exchange_credential_changes_affect_new_snapshots_only() {
    let (_root, paths, config) = fixture();
    plugins::configure(&config, &paths, ID, json!({"api_key":"first-key"})).unwrap();
    plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Declared).unwrap();
    let first = discover(&config, &paths, ID);
    let revision = first.revision().unwrap();
    let host = Arc::new(QueryHost::new(&[Ok((
        200,
        r#"{"result":"success","conversion_rates":{"CNY":7}}"#,
    ))]));
    let runtime = PluginRuntime::load(
        first.runtime_package(),
        first.settings().clone(),
        first.grants(),
        host.clone(),
    )
    .unwrap();
    plugins::configure(&config, &paths, ID, json!({"api_key":"second-key"})).unwrap();
    let second = discover(&config, &paths, ID);
    assert_ne!(second.revision().unwrap(), revision);
    assert_eq!(first.revision().unwrap(), revision);
    assert_eq!(second.settings()["api_key"], "second-key");
    assert_eq!(
        runtime
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

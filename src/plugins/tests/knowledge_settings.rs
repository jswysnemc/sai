use super::knowledge_support::*;
use crate::{
    config::ProviderApiKey,
    paths::SaiPaths,
    plugins::{self, config::load_config, discovery::PluginDescriptor, GrantChanges, GrantUpdate},
};
use serde_json::{json, Value};

/// 【知识库设置测试】【发现快照】输入主配置和隔离目录；返回真实内置兼容描述符
fn find(config: &crate::config::AppConfig, paths: &SaiPaths) -> PluginDescriptor {
    plugins::discovery::find(config, paths, "knowledge-base").unwrap()
}

/// 【知识库设置测试】【供应商隔离】只投影选中供应商及已解析凭据，管理设置不保存派生密钥
/// @returns 无；多密钥、环境引用、私密配置和外部同名包均遵循原凭据规则
#[test]
fn knowledge_credentials_are_resolved_only_for_selected_provider_and_not_persisted() {
    const ENV: &str = "SAI_KNOWLEDGE_EMBEDDING_TEST_KEY";
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = embedding_config();
    let mut unrelated = config.providers[0].clone();
    unrelated.id = "unrelated".into();
    unrelated.api_key = Some("unrelated-private-key".into());
    config.providers.push(unrelated);
    std::env::set_var(ENV, "environment-fixture-key");
    config.providers[0].api_key = Some(format!("$env:{ENV}"));
    assert_eq!(
        find(&config, &paths).settings()["provider"]["api_key"],
        "environment-fixture-key"
    );
    std::env::remove_var(ENV);
    config.providers[0].api_keys = vec![
        ProviderApiKey {
            id: "a".into(),
            api_key: "first-fixture-key".into(),
            label: String::new(),
        },
        ProviderApiKey {
            id: "b".into(),
            api_key: "selected-fixture-key".into(),
            label: String::new(),
        },
    ];
    config.providers[0].api_key_selected = Some("b".into());
    let selected = find(&config, &paths);
    assert_eq!(
        selected.settings()["provider"]["api_key"],
        "selected-fixture-key"
    );
    assert!(!selected
        .settings()
        .to_string()
        .contains("unrelated-private-key"));
    plugins::configure(
        &config,
        &paths,
        "knowledge-base",
        json!({"max_search_results":3}),
    )
    .unwrap();
    plugins::set_enabled(
        &config,
        &paths,
        "knowledge-base",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let stored = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
    for key in [
        "selected-fixture-key",
        "first-fixture-key",
        "unrelated-private-key",
        "environment-fixture-key",
    ] {
        assert!(!stored.contains(key));
    }
    assert_eq!(
        load_config(&paths).unwrap().plugins["knowledge-base"].settings,
        json!({"max_search_results":3})
    );
    config.providers[0].api_keys.clear();
    config.providers[0].api_key = None;
    std::fs::write(
        &paths.secrets_file,
        json!({"api_keys":{"embedding-test":"secret-file-fixture-key"}}).to_string(),
    )
    .unwrap();
    assert_eq!(
        find(&config, &paths).settings()["provider"]["api_key"],
        "secret-file-fixture-key"
    );
    let mut external = selected;
    external.source = plugins::PluginSource::Installed(root.path().join("external"));
    external.refresh_compatibility(&config, &paths).unwrap();
    assert_eq!(external.settings(), &json!({}));
}

/// 【知识库设置测试】【固定显式授权】更换库目录或供应商地址不会继承旧范围，停用再启用保留撤权
/// @returns 无；设置中的新地址始终与旧授权求交集
#[test]
fn knowledge_explicit_grants_do_not_expand_when_paths_or_endpoints_change() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = embedding_config();
    config.plugins.knowledge_base.data_dir = root.path().join("original").display().to_string();
    plugins::set_enabled(
        &config,
        &paths,
        "knowledge-base",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    config.providers[0].base_url = "https://new-embedding.test/v1".into();
    plugins::configure(
        &config,
        &paths,
        "knowledge-base",
        json!({"data_dir":root.path().join("changed")}),
    )
    .unwrap();
    let descriptor = find(&config, &paths);
    let effective = descriptor.grants().intersection(descriptor.capabilities());
    assert!(effective.http.is_empty());
    assert!(effective.system.read_paths.is_empty());
    assert!(effective.binary.write_paths.is_empty());
    plugins::set_enabled(
        &config,
        &paths,
        "knowledge-base",
        true,
        GrantUpdate::Changes(GrantChanges {
            plugin_storage: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    for enabled in [false, true] {
        plugins::set_enabled(
            &config,
            &paths,
            "knowledge-base",
            enabled,
            GrantUpdate::Keep,
        )
        .unwrap();
    }
    assert!(!find(&config, &paths).grants().system.plugin_storage);
}

/// 【知识库设置测试】【无效配置】业务字段和类型必须在配置保存前校验，失败保留原设置文件
/// @returns 无；数值、未知字段及输入路径错误均不固化
#[test]
fn knowledge_invalid_settings_do_not_replace_saved_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = embedding_config();
    plugins::configure(
        &config,
        &paths,
        "knowledge-base",
        json!({"max_search_results":3}),
    )
    .unwrap();
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    for value in [
        Value::Null,
        json!([]),
        json!({"unknown":true}),
        json!({"max_search_results":0}),
        json!({"upload_tool_enabled":"false"}),
        json!({"input_paths":"outside"}),
        json!({"index_max_bytes":100}),
        json!({"semantic_chunk_overlap":512}),
    ] {
        assert!(
            plugins::configure(&config, &paths, "knowledge-base", value.clone()).is_err(),
            "{value}"
        );
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
}

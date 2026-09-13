use super::{example_support, knowledge_support::embedding_config};
use crate::{
    config::AppConfig,
    paths::SaiPaths,
    plugins::{
        self, discovery::PluginDescriptor, private::PrivatePluginHost, GrantChanges, GrantUpdate,
    },
};
use sai_plugin_runtime::{InvocationContext, PluginPackage, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

/// 【知识库设置测试】【调用凭据】只追加测试命令，使用真实公共环境宿主
/// @param paths 隔离目录；descriptor 为当前安装设置及授权
/// @returns 可以观察固定测试凭据的运行时
fn probe(paths: &SaiPaths, descriptor: PluginDescriptor) -> PluginRuntime {
    let package = descriptor.runtime_package();
    let mut sources = package.sources().clone();
    sources.get_mut("init.lua").unwrap().push_str(r#"
        sai.register_command({name="probe", description="Test credentials", access="read_only", execute=function()
            return require("embedding.client").provider(require("settings").resolve(sai.config)).api_key
        end})
    "#);
    let host = Arc::new(PrivatePluginHost::for_descriptor(paths, &descriptor).unwrap());
    PluginRuntime::load(
        PluginPackage::new(package.manifest, sources).unwrap(),
        descriptor.settings().clone(),
        descriptor.grants(),
        host,
    )
    .unwrap()
}

/// 【知识库设置测试】【凭据隔离】独立配置不继承主供应商或私密配置，环境值在调用时读取
/// @returns 无；授权撤销后不能读取环境，配置文件不固化环境密钥
#[tokio::test]
async fn knowledge_credentials_use_only_explicit_settings_and_current_granted_environment() {
    const ENV: &str = "SAI_KNOWLEDGE_EMBEDDING_TEST_KEY";
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.providers[0].api_key = Some("unrelated-main-key".into());
    example_support::install_custom("knowledge-base", &paths, |manifest| {
        manifest.capabilities.system.environment.insert(ENV.into());
    });
    let mut settings = embedding_config();
    settings["provider"] =
        json!({"endpoint":"https://embedding.test/v1/embeddings","api_key_env":ENV});
    plugins::configure(&config, &paths, "knowledge-base", settings.clone()).unwrap();
    plugins::set_enabled(
        &config,
        &paths,
        "knowledge-base",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
    let find = || plugins::discovery::find(&config, &paths, "knowledge-base").unwrap();
    assert_eq!(find().settings(), &settings);
    let runtime = probe(&paths, find());
    for value in ["first-fixture-key", "changed-fixture-key"] {
        std::env::set_var(ENV, value);
        assert_eq!(
            runtime
                .call_command("probe", "", InvocationContext::default())
                .await
                .unwrap(),
            value
        );
    }
    plugins::set_enabled(
        &config,
        &paths,
        "knowledge-base",
        true,
        GrantUpdate::Changes(GrantChanges {
            environment: Some(Default::default()),
            ..Default::default()
        }),
    )
    .unwrap();
    let denied = probe(&paths, find())
        .call_command("probe", "", InvocationContext::default())
        .await
        .unwrap_err();
    std::env::remove_var(ENV);
    assert!(format!("{denied:#}").contains("environment grant"));
    let stored = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
    for key in [
        "unrelated-main-key",
        "first-fixture-key",
        "changed-fixture-key",
    ] {
        assert!(!stored.contains(key));
    }
}

/// 【知识库设置测试】【固定授权】更换目录或端点不改变清单，启停也不恢复撤权
/// @returns 无；只有显式更新清单与授权才能扩大范围
#[test]
fn knowledge_explicit_grants_do_not_expand_when_paths_or_endpoints_change() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    example_support::install_enabled("knowledge-base", &config, &paths);
    let before = plugins::discovery::find(&config, &paths, "knowledge-base").unwrap();
    plugins::configure(&config, &paths, "knowledge-base", json!({
        "data_dir":root.path().join("changed"), "provider":{"endpoint":"https://changed.test/embeddings"}
    })).unwrap();
    let after = plugins::discovery::find(&config, &paths, "knowledge-base").unwrap();
    assert_eq!(before.capabilities(), after.capabilities());
    assert_eq!(before.grants(), after.grants());
    assert!(!after.capabilities().http.contains("https://changed.test"));
    assert!(!after
        .capabilities()
        .binary
        .write_paths
        .contains(root.path().join("changed").to_str().unwrap()));
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
    assert!(
        !plugins::discovery::find(&config, &paths, "knowledge-base")
            .unwrap()
            .grants()
            .system
            .plugin_storage
    );
}

/// 【知识库设置测试】【无效配置】错误在保存前返回，保留原设置和授权
/// @returns 无；类型、路径、供应商和数值边界均明确校验
#[test]
fn knowledge_invalid_settings_do_not_replace_saved_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    example_support::install("knowledge-base", &paths);
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
        json!({"input_paths":["outside"]}),
        json!({"index_max_bytes":100}),
        json!({"semantic_chunk_overlap":512}),
        json!({"data_dir":"../outside"}),
        json!({"provider":{"endpoint":"file:///private"}}),
        json!({"provider":{"api_key_env":"bad name"}}),
        json!({"provider":[]}),
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

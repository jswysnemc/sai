use super::support::FixtureHost;
use crate::{config::AppConfig, paths::SaiPaths};
use sai_plugin_runtime::{PluginRuntime, ToolAccess};
use serde_json::json;
use std::sync::Arc;

const READ_TOOLS: [&str; 3] = ["search_meme", "show_meme", "recent_meme"];
const WRITE_TOOLS: [&str; 3] = ["add_meme", "update_meme", "delete_meme"];

/// 【表情迁移测试】【完整发布】六个公开工具必须由同一实际发布的 Lua 包提供
/// @returns 无；读取与写入工具保留原名称和权限分类
#[test]
fn memes_publish_all_six_tools_as_lua() {
    let package = super::example_support::package("memes");
    let grants = package.manifest.capabilities.clone();
    let runtime =
        PluginRuntime::load(package, json!({}), grants, Arc::new(FixtureHost::default())).unwrap();
    assert_eq!(runtime.tools().len(), 6);
    for (names, access) in [
        (READ_TOOLS, ToolAccess::ReadOnly),
        (WRITE_TOOLS, ToolAccess::Writes),
    ] {
        for name in names {
            let tool = runtime
                .tools()
                .iter()
                .find(|tool| tool.name == name)
                .unwrap();
            assert_eq!(tool.access, access, "{name}");
        }
    }
}

/// 【表情迁移测试】【真实归属】普通和只读目录使用已安装插件与独立开关
/// @returns 无；写入工具不进入只读目录，禁用时六个工具均不可调用
#[test]
fn memes_registry_uses_installed_lua_and_explicit_enablement() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("memes", &paths);
    for enabled in [true, false] {
        crate::plugins::set_enabled(
            &config,
            &paths,
            "memes",
            enabled,
            crate::plugins::GrantUpdate::Declared,
        )
        .unwrap();
        let normal = crate::tools::builtin_registry_without_mcp(&config, &paths);
        let readonly = crate::tools::readonly_registry(&config, &paths);
        assert!(normal.plugin_diagnostics().is_empty());
        assert!(readonly.plugin_diagnostics().is_empty());
        for name in READ_TOOLS.into_iter().chain(WRITE_TOOLS) {
            let public = format!("lua__memes__{name}");
            assert_eq!(normal.contains(&public), enabled, "{name}");
            assert_eq!(
                readonly.contains(&public),
                enabled && READ_TOOLS.contains(&name),
                "{name}"
            );
            if enabled {
                assert_eq!(normal.plugin_owner(&public), Some("memes"), "{name}");
            }
        }
    }
}

/// 【表情设置测试】【旧配置隔离】主配置旧字段不能进入外部包，独立设置原文保持不变
/// @returns 无；不自动采用用户的旧图库目录和偏好
#[test]
fn memes_settings_override_legacy_values_without_rewriting_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut legacy = serde_json::to_value(AppConfig::default()).unwrap();
    legacy["plugins"]["memes"] = json!({"enabled":true,"width_percent":51,"height_percent":19,"libraries":{"default":"legacy"}});
    let config: AppConfig = serde_json::from_value(legacy).unwrap();
    super::example_support::install("memes", &paths);
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    let file = paths.config_dir.join("plugins.jsonc");
    let bytes = serde_json::to_vec(
        &json!({"plugins":{"memes":{"enabled":true,"settings":{"width_percent":23}}}}),
    )
    .unwrap();
    std::fs::write(&file, &bytes).unwrap();
    let found = crate::plugins::discover(&config, &paths);
    let descriptor = found
        .plugins
        .iter()
        .find(|plugin| plugin.package.manifest.id == "memes")
        .unwrap();
    assert_eq!(descriptor.settings()["width_percent"], 23);
    assert_eq!(descriptor.settings(), &json!({"width_percent":23}));
    assert_eq!(
        descriptor.grants(),
        sai_plugin_runtime::Capabilities::default()
    );
    let registry = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert!(registry.plugin_diagnostics().is_empty());
    assert!(registry.contains("lua__memes__add_meme"));
    assert_eq!(std::fs::read(file).unwrap(), bytes);
}

/// 【表情兼容测试】【目录变更】重新配置目录不扩大已有显式授权，重新授权前不能写入新库
/// @returns 无；新路径进入声明但没有有效写入、永久删除或回收权限
#[tokio::test]
async fn memes_directory_changes_do_not_expand_existing_explicit_grants() {
    let root = tempfile::tempdir().unwrap();
    let config = AppConfig::default();
    let mut descriptor = super::memes_support::descriptor(root.path(), &config);
    descriptor.setting.grants = Some(descriptor.grants());
    let changed = root.path().join("changed");
    descriptor.setting.settings["user_dir"] = json!(changed);
    let effective = descriptor.capabilities().intersection(&descriptor.grants());
    for directories in [
        &effective.binary.write_paths,
        &effective.system.remove_paths,
        &effective.system.trash_paths,
    ] {
        assert!(!directories.contains(changed.to_str().unwrap()));
    }
    let runtime = PluginRuntime::load(
        descriptor.runtime_package(),
        descriptor.settings().clone(),
        descriptor.grants(),
        super::memes_support::MemeHost::new(root.path()),
    )
    .unwrap();
    assert!(runtime
        .call_tool(
            "add_meme",
            super::memes_support::addition(root.path(), 1),
            super::memes_support::context(root.path())
        )
        .await
        .is_err());
    assert!(!changed.exists());
}

/// 【表情兼容测试】【来源隔离】外部来源不能从内置 ID 或旧配置继承目录和默认授权
/// @returns 无；只有包自己的设置及声明可见，缺省授权为空
#[test]
fn memes_external_sources_do_not_inherit_builtin_compatibility() {
    let root = tempfile::tempdir().unwrap();
    let config = AppConfig::default();
    let mut descriptor = super::memes_support::descriptor(root.path(), &config);
    descriptor.source =
        crate::plugins::discovery::PluginSource::Installed(root.path().join("installed"));
    descriptor.setting.settings = json!({});
    descriptor.setting.grants = None;
    assert_eq!(descriptor.settings(), &json!({}));
    assert_eq!(
        descriptor.capabilities(),
        &descriptor.package.manifest.capabilities
    );
    assert_eq!(
        descriptor.grants(),
        sai_plugin_runtime::Capabilities::default()
    );
}

/// 【表情兼容测试】【无效设置】错误类型、越界尺寸、未知字段和空目录不能静默回退
/// @returns 无；坏设置产生诊断并且不注册六个业务工具
#[test]
fn memes_invalid_settings_fail_without_defaulting_to_broader_permissions() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    super::example_support::install("memes", &paths);
    std::fs::create_dir_all(&paths.config_dir).unwrap();
    for settings in [
        json!({"width_percent":256}),
        json!({"max_image_mb":-1}),
        json!({"unknown":true}),
        json!({"builtin_dirs":[]}),
        json!({"input_paths":true}),
    ] {
        std::fs::write(
            paths.config_dir.join("plugins.jsonc"),
            json!({"plugins":{"memes":{"enabled":true,"settings":settings}}}).to_string(),
        )
        .unwrap();
        let registry = crate::tools::builtin_registry_without_mcp(&AppConfig::default(), &paths);
        assert!(!registry.plugin_diagnostics().is_empty(), "{settings}");
        for name in READ_TOOLS.into_iter().chain(WRITE_TOOLS) {
            assert!(!registry.contains(name));
        }
    }
}

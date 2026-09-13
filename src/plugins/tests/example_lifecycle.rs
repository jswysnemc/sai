use super::example_support::{directory, install, registry};
use super::support::FixtureHost;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::discovery::find;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use sai_plugin_runtime::{Capabilities, PluginPackage};
use serde_json::json;
use std::sync::Arc;

/// 【示例插件测试】【逐包管理】每个提取包均经过普通检查、安装、启用、更新和撤权
/// @returns 无；包可独立装卸，工具命名与权限不存在内置回退
#[test]
fn every_extracted_package_uses_the_external_lifecycle() {
    for id in super::example_support::EXTRACTED_IDS {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let config = AppConfig::default();
        assert!(find(&config, &paths, id).is_err());
        let checked = plugins::validate_package(&directory(id)).unwrap();
        install(id, &paths);
        let initial = find(&config, &paths, id).unwrap();
        assert!(!initial.setting.enabled);
        assert_eq!(initial.grants(), Capabilities::default());
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
        let active = registry(&config, &paths, id, Arc::new(FixtureHost::default()));
        for tool in checked.tools {
            let name = format!("lua__{id}__{}", tool.name);
            assert_eq!(active.plugin_owner(&name), Some(*id));
            assert!(!active.contains(&tool.name));
        }
        plugins::configure(&config, &paths, id, json!({})).unwrap();
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Declared).unwrap();
        let before = find(&config, &paths, id).unwrap();
        plugins::install(&directory(id), &paths, true).unwrap();
        let updated = find(&config, &paths, id).unwrap();
        assert_eq!(updated.grants(), before.grants());
        assert_eq!(updated.settings(), before.settings());
        assert!(updated.setting.enabled);
        plugins::set_enabled(
            &config,
            &paths,
            id,
            true,
            GrantUpdate::Changes(GrantChanges {
                http: Some(Default::default()),
                http_read_any: Some(false),
                http_read_only_post: Some(Default::default()),
                environment: Some(Default::default()),
                model: Some(false),
                vision: Some(false),
                notifications: Some(false),
                reply_policy: Some(false),
                tools: Some(Default::default()),
                read_paths: Some(Default::default()),
                remove_paths: Some(Default::default()),
                trash_paths: Some(Default::default()),
                processes: Some(Default::default()),
                session_storage: Some(false),
                plugin_storage: Some(false),
                notify: Some(false),
                schedule: Some(false),
                workspace: Some(false),
                public_downloads: Some(false),
                write_paths: Some(Default::default()),
                display_images: Some(false),
                ..Default::default()
            }),
        )
        .unwrap();
        plugins::set_enabled(&config, &paths, id, false, GrantUpdate::Keep).unwrap();
        assert!(!find(&config, &paths, id).unwrap().setting.enabled);
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
        assert_eq!(
            find(&config, &paths, id).unwrap().grants(),
            Capabilities::default()
        );
        plugins::remove(&config, &paths, id).unwrap();
        assert!(find(&config, &paths, id).is_err());
    }
}

/// 【示例插件测试】【默认隔离】旧开关与默认 Agent 均不能恢复已提取业务工具
/// @returns 无；普通和只读入口均只保留真实可用工具
#[test]
fn extracted_examples_require_installation_even_with_legacy_switches() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut legacy = serde_json::to_value(AppConfig::default()).unwrap();
    legacy["plugins"]["weather"] = json!({"enabled":true});
    legacy["plugins"]["hash_codec"] = json!({"enabled":true});
    let config: AppConfig = serde_json::from_value(legacy).unwrap();
    let catalog = crate::tools::tool_catalog(&config, &paths);
    for (id, local) in [
        ("hash-codec", "calculate_hash"),
        ("hash-codec", "decode_encoded_text"),
        ("weather", "get_weather"),
    ] {
        assert!(find(&config, &paths, id).is_err());
        assert!(plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Declared).is_err());
        for name in [local.to_string(), format!("lua__{id}__{local}")] {
            assert!(!catalog.iter().any(|entry| entry.name == name));
            for tools in [
                crate::tools::builtin_registry_without_mcp(&config, &paths),
                crate::tools::readonly_registry(&config, &paths),
            ] {
                assert!(!tools.contains(&name));
                assert!(tools.contains("read_file"));
                assert!(tools.plugin_diagnostics().is_empty());
            }
            for profile in crate::config::seed_default_agent_profiles() {
                assert!(!profile.enabled_tools.contains(&name));
            }
        }
    }
}

/// 【示例插件测试】【独立生命周期】零权限与网络示例使用普通安装、更新和卸载事务
/// @returns 无；源码更新和重新启用都不能恢复网络授权
#[tokio::test]
async fn extracted_examples_install_run_update_revoke_and_remove() {
    for (id, local, args, expected) in [
        (
            "hash-codec",
            "calculate_hash",
            json!({"input_text":"abc"}),
            "ba7816bf",
        ),
        (
            "weather",
            "get_weather",
            json!({"location":"Beijing"}),
            "Clear +20°C",
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(&root.path().join("app"));
        let config = AppConfig::default();
        let name = format!("lua__{id}__{local}");
        let host = Arc::new(FixtureHost::new(&[(200, "Clear +20°C")]));
        let inspection = plugins::validate_package(&directory(id)).unwrap();
        assert_eq!(inspection.manifest.id, id);
        let packed =
            plugins::pack(&directory(id), Some(&root.path().join("example.tar.gz"))).unwrap();
        assert!(packed.path.is_file());
        install(id, &paths);
        let installed = find(&config, &paths, id).unwrap();
        assert!(!installed.setting.enabled);
        assert_eq!(installed.grants(), Capabilities::default());
        assert!(!registry(&config, &paths, id, host.clone()).contains(&name));

        // 1. 【示例插件测试】【显式启用】无网络授权时天气请求不得到达宿主
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
        let ungranted = registry(&config, &paths, id, host.clone());
        if id == "weather" {
            assert!(ungranted.call(&name, &args.to_string()).await.is_err());
            assert!(host.requests.lock().unwrap().is_empty());
            plugins::set_enabled(
                &config,
                &paths,
                id,
                true,
                GrantUpdate::Changes(GrantChanges {
                    http: Some(["https://wttr.in".into()].into()),
                    ..Default::default()
                }),
            )
            .unwrap();
        }
        plugins::configure(&config, &paths, id, json!({})).unwrap();
        let active = registry(&config, &paths, id, host.clone());
        assert!(!active.contains(local));
        assert_eq!(active.plugin_owner(&name), Some(id));
        assert!(active
            .call(&name, &args.to_string())
            .await
            .unwrap()
            .contains(expected));
        assert!(active.call(&name, r#"{"unexpected":true}"#).await.is_err());

        // 2. 【示例插件测试】【更新边界】更新源码声明不会授予新增来源
        let source = root.path().join("update");
        std::fs::create_dir_all(&source).unwrap();
        let mut package = PluginPackage::from_directory(&directory(id)).unwrap();
        package.manifest.version = "1.0.1".into();
        package
            .manifest
            .capabilities
            .http
            .insert("https://new.example.test".into());
        std::fs::write(
            source.join("sai-plugin.json"),
            package.manifest_json().unwrap(),
        )
        .unwrap();
        for (path, contents) in package.sources() {
            std::fs::write(source.join(path), contents).unwrap();
        }
        plugins::install(&source, &paths, true).unwrap();
        let updated = find(&config, &paths, id).unwrap();
        assert_eq!(updated.package.manifest.version, "1.0.1");
        assert!(updated.setting.enabled);
        assert!(!updated.grants().http.contains("https://new.example.test"));

        // 3. 【示例插件测试】【撤权卸载】撤权保持到重新启用和重新安装
        plugins::set_enabled(
            &config,
            &paths,
            id,
            true,
            GrantUpdate::Changes(GrantChanges {
                http: Some(Default::default()),
                ..Default::default()
            }),
        )
        .unwrap();
        plugins::set_enabled(&config, &paths, id, false, GrantUpdate::Keep).unwrap();
        assert!(!registry(&config, &paths, id, host.clone()).contains(&name));
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
        let revoked = registry(&config, &paths, id, host.clone());
        if id == "weather" {
            assert!(revoked.call(&name, &args.to_string()).await.is_err());
            assert_eq!(host.requests.lock().unwrap().len(), 1);
        } else {
            assert!(revoked.call(&name, &args.to_string()).await.is_ok());
        }
        plugins::remove(&config, &paths, id).unwrap();
        assert!(find(&config, &paths, id).is_err());
        let saved = load_config(&paths).unwrap().plugins[id].clone();
        assert!(!saved.enabled);
        assert_eq!(saved.grants, Some(Capabilities::default()));
        install(id, &paths);
        assert!(!find(&config, &paths, id).unwrap().setting.enabled);
        assert_eq!(
            find(&config, &paths, id).unwrap().grants(),
            Capabilities::default()
        );
    }
}

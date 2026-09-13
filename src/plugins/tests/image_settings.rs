use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use serde_json::json;

/// 【图片设置测试】【独立配置】旧主配置不注入凭据、地址或显示偏好，显式 false 原样保存
/// @returns 无；两个包只读取各自独立设置
#[test]
fn explicit_image_settings_do_not_inherit_legacy_defaults_or_credentials() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut legacy = serde_json::to_value(AppConfig::default()).unwrap();
    legacy["plugins"]["image_generation"] = json!({"api_keys":["legacy-secret"],
        "base_url":"https://legacy-image.test","output_dir":"legacy-output","auto_print":true});
    legacy["plugins"]["print_image"] = json!({"width_percent":80});
    let config: AppConfig = serde_json::from_value(legacy).unwrap();
    for id in ["image-generation", "image-display"] {
        super::example_support::install(id, &paths);
    }
    let explicit = json!({"auto_print":false,"model":"custom-image"});
    plugins::configure(&config, &paths, "image-generation", explicit.clone()).unwrap();
    let found = find(&config, &paths, "image-generation").unwrap();
    assert_eq!(found.settings(), &explicit);
    assert_eq!(
        found.capabilities().http,
        ["https://api.openai.com".into()].into()
    );
    assert_eq!(
        found.capabilities().binary.write_paths,
        ["~/Pictures/sai/generated-images".into()].into()
    );
    assert_eq!(found.grants(), Default::default());
    plugins::configure(
        &config,
        &paths,
        "image-display",
        json!({"width_percent":20}),
    )
    .unwrap();
    assert_eq!(
        find(&config, &paths, "image-display").unwrap().settings(),
        &json!({"width_percent":20})
    );
    let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
    for private in ["legacy-secret", "legacy-output", "legacy-image.test"] {
        assert!(!saved.contains(private));
    }
}

/// 【图片设置测试】【入口一致】生成仍要求写入模式，显示可在只读表中注册，启停完全独立
/// @returns 无；未安装或禁用的包不提供工具
#[test]
fn image_plugins_use_common_registry_permissions_and_activation_rules() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    for (id, local) in [
        ("image-generation", "generate_image"),
        ("image-display", "print_image"),
    ] {
        let name = format!("lua__{id}__{local}");
        assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(&name));
        super::example_support::install(id, &paths);
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
    }
    let common = crate::tools::builtin_registry_without_mcp(&config, &paths);
    let readonly = crate::tools::readonly_registry(&config, &paths);
    assert_eq!(
        common.plugin_owner("lua__image-generation__generate_image"),
        Some("image-generation")
    );
    assert_eq!(
        common.plugin_owner("lua__image-display__print_image"),
        Some("image-display")
    );
    assert!(readonly.contains("lua__image-display__print_image"));
    assert!(!readonly.contains("lua__image-generation__generate_image"));
    assert!(common.plugin_diagnostics().is_empty());
    assert!(readonly.plugin_diagnostics().is_empty());
    for (id, local) in [
        ("image-generation", "generate_image"),
        ("image-display", "print_image"),
    ] {
        plugins::set_enabled(&config, &paths, id, false, GrantUpdate::Keep).unwrap();
        assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths)
            .contains(&format!("lua__{id}__{local}")));
    }
}

/// 【图片设置测试】【原子校验】非法类型、未知字段和无效路径不替换已有设置
/// @returns 无；校验失败保留原始配置文件
#[test]
fn invalid_image_settings_leave_configuration_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    for id in ["image-generation", "image-display"] {
        super::example_support::install(id, &paths);
    }
    plugins::configure(
        &config,
        &paths,
        "image-generation",
        json!({"auto_print":false}),
    )
    .unwrap();
    let original = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    for (id, invalid) in [
        ("image-generation", json!({"auto_print":null})),
        ("image-generation", json!({"unknown":true})),
        ("image-generation", json!({"timeout_seconds":601})),
        ("image-generation", json!({"api_keys":[3]})),
        ("image-generation", json!({"output_dir":"../outside"})),
        ("image-generation", json!({"base_url":"file:///tmp"})),
        (
            "image-generation",
            json!({"base_url":"https://user:secret@image.test"}),
        ),
        ("image-display", json!({"width_percent":-1})),
        ("image-display", json!({"height_percent":null})),
        ("image-display", json!({"language":"invalid"})),
    ] {
        assert!(
            plugins::configure(&config, &paths, id, invalid.clone()).is_err(),
            "{invalid}"
        );
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            original
        );
    }
}

/// 【图片设置测试】【来源隔离】相同标识的外部源码不能取得任何专用设置或权限
/// @returns 无；描述符只保留显式设置和自己的清单
#[test]
fn external_packages_never_inherit_image_compatibility_settings() {
    for id in ["image-generation", "image-display", "own-image"] {
        let mut plugin = super::support::descriptor(id, "");
        plugin.setting.settings = json!({"own":true});
        assert_eq!(plugin.settings(), &json!({"own":true}));
        assert!(plugin.capabilities().binary.is_empty());
        assert_eq!(plugin.grants(), Default::default());
    }
}

/// 【图片设置测试】【固定授权】新地址和目录不会获得权限，撤销下载权限保持有效
/// @returns 无；设置更新不改动清单或恢复授权
#[test]
fn image_grants_are_separate_and_do_not_follow_new_paths() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install_enabled("image-generation", &config, &paths);
    let before_grants = find(&config, &paths, "image-generation").unwrap().grants();
    plugins::set_enabled(
        &config,
        &paths,
        "image-generation",
        true,
        GrantUpdate::Changes(GrantChanges {
            public_downloads: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    plugins::configure(
        &config,
        &paths,
        "image-generation",
        json!({"output_dir":"new-output","base_url":"https://new-image.test"}),
    )
    .unwrap();
    let found = find(&config, &paths, "image-generation").unwrap();
    let effective = found.capabilities().intersection(&found.grants());
    assert!(!effective.binary.public_downloads);
    assert_eq!(
        effective.binary.write_paths,
        before_grants.binary.write_paths
    );
    assert_eq!(effective.http, before_grants.http);
    assert!(!effective.binary.write_paths.contains("new-output"));
    assert!(!effective.http.contains("https://new-image.test"));
    assert!(effective.tools.contains("lua__image-display__print_image"));
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "image-generation",
        true,
        GrantUpdate::Changes(GrantChanges {
            write_paths: Some(["new-output".into()].into()),
            ..Default::default()
        })
    )
    .is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        before
    );
}

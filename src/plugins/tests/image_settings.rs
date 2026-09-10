use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::find;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use serde_json::json;

/// 【图片设置测试】【字段覆盖】显式 false 生效，其他字段继续采用最新旧设置，凭据不写回插件配置。
#[test]
fn explicit_image_settings_override_legacy_defaults_without_persisting_credentials() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.image_generation.auto_print = true;
    config.plugins.image_generation.base_url = "https://legacy-image.test".into();
    config.plugins.image_generation.output_dir = "legacy-output".into();
    config.plugins.image_generation.api_keys = vec!["legacy-fixture-secret".into()];
    plugins::configure(
        &config,
        &paths,
        "image-generation",
        json!({"auto_print":false,"model":"custom-image"}),
    )
    .unwrap();
    config.plugins.image_generation.default_resolution = "4K".into();
    let found = find(&config, &paths, "image-generation").unwrap();
    assert_eq!(found.settings()["auto_print"], false);
    assert_eq!(found.settings()["model"], "custom-image");
    assert_eq!(found.settings()["default_resolution"], "4K");
    assert_eq!(
        found.capabilities().http,
        ["https://legacy-image.test".into()].into()
    );
    assert_eq!(
        found.capabilities().binary.write_paths,
        ["legacy-output".into()].into()
    );
    let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
    for private in [
        "legacy-fixture-secret",
        "legacy-output",
        "legacy-image.test",
        "4K",
    ] {
        assert!(!saved.contains(private));
    }
    config.plugins.print_image.width_percent = 45;
    config.plugins.print_image.height_percent = 35;
    plugins::configure(
        &config,
        &paths,
        "image-display",
        json!({"width_percent":20}),
    )
    .unwrap();
    config.plugins.print_image.height_percent = 50;
    let display = find(&config, &paths, "image-display").unwrap();
    assert_eq!(display.settings()["width_percent"], 20);
    assert_eq!(display.settings()["height_percent"], 50);
}

/// 【图片设置测试】【入口一致】生成工具保持写入属性，显示工具在只读工具表中可用，显式启停优先于旧开关。
#[test]
fn image_plugins_use_common_registry_permissions_and_activation_rules() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.image_generation.enabled = false;
    config.plugins.print_image.enabled = false;
    for (id, tool) in [
        ("image-generation", "generate_image"),
        ("image-display", "print_image"),
    ] {
        assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(tool));
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Keep).unwrap();
    }
    let common = crate::tools::builtin_registry_without_mcp(&config, &paths);
    let readonly = crate::tools::readonly_registry(&config, &paths);
    assert_eq!(
        common.plugin_owner("generate_image"),
        Some("image-generation")
    );
    assert_eq!(common.plugin_owner("print_image"), Some("image-display"));
    assert!(readonly.contains("print_image"));
    assert!(!readonly.contains("generate_image"));
    assert!(common.plugin_diagnostics().is_empty());
    assert!(readonly.plugin_diagnostics().is_empty());
    config.plugins.image_generation.enabled = true;
    config.plugins.print_image.enabled = true;
    for (id, tool) in [
        ("image-generation", "generate_image"),
        ("image-display", "print_image"),
    ] {
        plugins::set_enabled(&config, &paths, id, false, GrantUpdate::Keep).unwrap();
        assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths).contains(tool));
    }
}

/// 【图片设置测试】【原子校验】非法类型、未知字段和无效路径不替换已有设置文件。
#[test]
fn invalid_image_settings_leave_configuration_unchanged() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
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
        assert!(plugins::configure(&config, &paths, id, invalid).is_err());
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            original
        );
    }
}

/// 【图片设置测试】【定向兼容】外部源码即使声明相同 ID，也不能继承旧图片凭据或显示偏好。
#[test]
fn external_packages_never_inherit_image_compatibility_settings() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.image_generation.api_keys = vec!["fixture-secret".into()];
    for id in ["image-generation", "image-display", "own-image"] {
        let mut plugin = super::support::descriptor(id, "");
        plugin.setting.settings = json!({"own":true});
        plugin.refresh_compatibility(&config, &paths).unwrap();
        assert_eq!(plugin.settings(), &json!({"own":true}));
        assert!(plugin.capabilities().binary.is_empty());
    }
}

/// 【图片设置测试】【独立撤权】修改 API 来源或目录不会重新授予先前撤销的能力。
#[test]
fn image_grants_are_separate_and_old_output_grants_do_not_follow_new_paths() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.plugins.image_generation.output_dir = "old-output".into();
    plugins::set_enabled(
        &config,
        &paths,
        "image-generation",
        true,
        GrantUpdate::Declared,
    )
    .unwrap();
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
        json!({"output_dir":"new-output"}),
    )
    .unwrap();
    let found = find(&config, &paths, "image-generation").unwrap();
    let effective = found.capabilities().intersection(&found.grants());
    assert!(!effective.binary.public_downloads);
    assert!(effective.binary.write_paths.is_empty());
    assert!(!effective.http.is_empty());
    assert!(effective.tools.contains("print_image"));
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "image-generation",
        true,
        GrantUpdate::Changes(GrantChanges {
            write_paths: Some(["outside".into()].into()),
            ..Default::default()
        })
    )
    .is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        before
    );
}

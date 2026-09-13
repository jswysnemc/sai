use crate::plugins::{self, discovery::find, GrantChanges, GrantUpdate};
use crate::tools::ToolPermission;
use crate::{config::AppConfig, paths::SaiPaths};
use serde_json::json;

/// 【搜图设置测试】【默认与覆盖】设置独立保存，显式 false 保留且清单路径不写回配置文件。
/// @returns 无；未覆盖字段由插件默认值提供
#[test]
fn explicit_web_image_settings_do_not_inherit_legacy_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("web-images", &paths);
    let explicit = json!({"auto_preview":false,"safe_search":false,"max_results":2});
    plugins::configure(&config, &paths, "web-images", explicit.clone()).unwrap();
    let found = find(&config, &paths, "web-images").unwrap();
    assert_eq!(found.settings(), &explicit);
    assert_eq!(
        found.capabilities().binary.write_paths,
        ["~/Pictures/sai/web-images".into()].into()
    );
    assert_eq!(found.grants(), Default::default());
    let saved = std::fs::read_to_string(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(!saved.contains("cache_dir"));
    assert!(!saved.contains("timeout_seconds"));
    assert!(!saved.contains("vision_provider_id"));
}

/// 【搜图设置测试】【双模式注册】普通工具表需要写入权限，只读工具表保留查询，独立设置控制启停。
/// @returns 无；两个入口均由真实 Lua 包提供
#[test]
fn web_images_keep_readonly_queries_and_explicit_activation_rules() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("web-images", &paths);
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths)
        .contains("lua__web-images__search_web_images"));
    plugins::set_enabled(&config, &paths, "web-images", true, GrantUpdate::Keep).unwrap();
    let common = crate::tools::builtin_registry_without_mcp(&config, &paths);
    let readonly = crate::tools::readonly_registry(&config, &paths);
    assert_eq!(
        common.plugin_owner("lua__web-images__search_web_images"),
        Some("web-images")
    );
    assert_eq!(
        readonly.plugin_owner("lua__web-images__search_web_images"),
        Some("web-images")
    );
    assert_eq!(
        common
            .permission("lua__web-images__search_web_images")
            .unwrap(),
        ToolPermission::Writes
    );
    assert_eq!(
        readonly
            .permission("lua__web-images__search_web_images")
            .unwrap(),
        ToolPermission::ReadOnly
    );
    assert!(
        common.plugin_diagnostics().is_empty(),
        "{:?}",
        common.plugin_diagnostics()
    );
    assert!(
        readonly.plugin_diagnostics().is_empty(),
        "{:?}",
        readonly.plugin_diagnostics()
    );
    plugins::set_enabled(&config, &paths, "web-images", false, GrantUpdate::Keep).unwrap();
    assert!(!crate::tools::builtin_registry_without_mcp(&config, &paths)
        .contains("lua__web-images__search_web_images"));
    assert!(!crate::tools::readonly_registry(&config, &paths)
        .contains("lua__web-images__search_web_images"));
    assert!(!crate::tools::tool_catalog(&config, &paths)
        .iter()
        .any(|tool| tool.name == "lua__web-images__search_web_images"));
}

/// 【搜图设置测试】【授权固定】显式授权不随缓存目录和搜索来源改变，视觉撤权不影响其他能力。
/// @returns 无；变更来源必须同时调整清单与明确授权
#[test]
fn explicit_grants_do_not_follow_changed_paths_or_origins() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("web-images", &paths);
    plugins::set_enabled(&config, &paths, "web-images", true, GrantUpdate::Declared).unwrap();
    plugins::set_enabled(
        &config,
        &paths,
        "web-images",
        true,
        GrantUpdate::Changes(GrantChanges {
            vision: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    plugins::configure(
        &config,
        &paths,
        "web-images",
        json!({"cache_dir":"new-output","duckduckgo_base_url":"https://new-search.test"}),
    )
    .unwrap();
    let found = find(&config, &paths, "web-images").unwrap();
    let effective = found.capabilities().intersection(&found.grants());
    assert!(!effective.vision);
    assert!(!effective.model);
    assert_eq!(
        effective.binary.write_paths,
        ["~/Pictures/sai/web-images".into()].into()
    );
    assert!(effective.binary.public_downloads);
    assert_eq!(
        effective.http,
        [
            "https://duckduckgo.com".into(),
            "https://www.bing.com".into()
        ]
        .into()
    );
    assert!(effective.tools.contains("lua__image-display__print_image"));
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    assert!(plugins::set_enabled(
        &config,
        &paths,
        "web-images",
        true,
        GrantUpdate::Changes(GrantChanges {
            model: Some(true),
            ..Default::default()
        })
    )
    .is_err());
    assert_eq!(
        std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
        before
    );
    plugins::set_enabled(
        &config,
        &paths,
        "web-images",
        true,
        GrantUpdate::Changes(GrantChanges {
            vision: Some(true),
            ..Default::default()
        }),
    )
    .unwrap();
    let found = find(&config, &paths, "web-images").unwrap();
    let effective = found.capabilities().intersection(&found.grants());
    assert!(effective.vision);
    assert_eq!(
        effective.binary.write_paths,
        ["~/Pictures/sai/web-images".into()].into()
    );
    assert_eq!(
        effective.http,
        [
            "https://duckduckgo.com".into(),
            "https://www.bing.com".into()
        ]
        .into()
    );
}

/// 【搜图设置测试】【原子校验】错误类型、未知字段和越界设置不能替换已保存的有效配置。
/// @returns 无；每次失败后文件内容保持一致
#[test]
fn invalid_web_image_settings_leave_saved_configuration_intact() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    super::example_support::install("web-images", &paths);
    plugins::configure(&config, &paths, "web-images", json!({"safe_search":false})).unwrap();
    let before = std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap();
    for invalid in [
        json!({"safe_search":null}),
        json!({"max_results":-1}),
        json!({"preview_count":1.5}),
        json!({"timeout_seconds":601}),
        json!({"max_download_mb":"4"}),
        json!({"language":"other"}),
        json!({"unknown":true}),
        json!({"cache_dir":"../outside"}),
        json!({"cache_dir":""}),
        json!({"duckduckgo_base_url":"file:///tmp"}),
        json!({"bing_base_url":"https://user:secret@search.test"}),
        json!({"bing_base_url":"https://search.test?token=value"}),
        json!({"bing_base_url":"https://search.test#fragment"}),
    ] {
        assert!(
            plugins::configure(&config, &paths, "web-images", invalid.clone()).is_err(),
            "{invalid}"
        );
        assert_eq!(
            std::fs::read(paths.config_dir.join("plugins.jsonc")).unwrap(),
            before
        );
    }
}

/// 【搜图设置测试】【外部隔离】同名外部包也不能继承旧设置、视觉配置或图片目录授权。
/// @returns 无；显式外部设置原样保留
#[test]
fn external_packages_do_not_inherit_web_image_compatibility() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    super::example_support::install("web-images", &paths);
    config.plugins.vision.vision_provider_id = "private-provider".into();
    let mut descriptor = super::support::descriptor("web-images", "");
    descriptor.setting.settings = json!({"own":true});
    assert_eq!(descriptor.settings(), &json!({"own":true}));
    assert!(!descriptor.capabilities().vision);
    assert!(descriptor.capabilities().binary.is_empty());
}

use crate::{config::AppConfig, paths::SaiPaths, plugins, tools};

/// 【搜图路径测试】【短目录注册】真实临时目录中的波浪号不妨碍普通包注册
/// @returns 无；两个工具表均使用明确配置与授权的目录
#[test]
fn short_named_picture_paths_register_in_both_registries() {
    let root = tempfile::Builder::new()
        .prefix("RUNNER~1-")
        .tempdir()
        .unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let cache = paths.pictures_dir.join("web-images");
    super::example_support::install_custom("web-images", &paths, |manifest| {
        manifest.capabilities.binary.write_paths = [cache.display().to_string()].into();
    });
    plugins::configure(
        &config,
        &paths,
        "web-images",
        serde_json::json!({"cache_dir":cache}),
    )
    .unwrap();
    super::example_support::install_enabled("web-images", &config, &paths);
    for registry in [
        tools::builtin_registry_without_mcp(&config, &paths),
        tools::readonly_registry(&config, &paths),
    ] {
        assert!(
            registry.plugin_diagnostics().is_empty(),
            "{:?}",
            registry.plugin_diagnostics()
        );
        assert_eq!(
            registry.plugin_owner("lua__web-images__search_web_images"),
            Some("web-images")
        );
    }
    let found = plugins::discovery::find(&config, &paths, "web-images").unwrap();
    assert_eq!(found.settings()["cache_dir"], serde_json::json!(cache));
}

/// 【搜图路径测试】【显式目录】目录中的波浪号保留原值，修改设置不产生新目录授权
/// @returns 无；管理操作不创建缓存目录，原授权仍只允许原目录
#[test]
fn explicit_tilde_cache_paths_keep_grants_bound_to_the_original_directory() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let original = root.path().join("cache~1");
    super::example_support::install_custom("web-images", &paths, |manifest| {
        manifest.capabilities.binary.write_paths = [original.display().to_string()].into();
    });
    plugins::configure(
        &config,
        &paths,
        "web-images",
        serde_json::json!({"cache_dir":original}),
    )
    .unwrap();
    super::example_support::install_enabled("web-images", &config, &paths);
    let found = plugins::discovery::find(&config, &paths, "web-images").unwrap();
    assert_eq!(found.settings()["cache_dir"], serde_json::json!(original));
    let granted = [original.display().to_string()].into();
    assert_eq!(
        found
            .capabilities()
            .intersection(&found.grants())
            .binary
            .write_paths,
        granted
    );
    let changed = root.path().join("cache~2");
    plugins::configure(
        &config,
        &paths,
        "web-images",
        serde_json::json!({"cache_dir":changed}),
    )
    .unwrap();
    let found = plugins::discovery::find(&config, &paths, "web-images").unwrap();
    assert_eq!(
        found
            .capabilities()
            .intersection(&found.grants())
            .binary
            .write_paths,
        granted
    );
    assert!(!found
        .grants()
        .binary
        .write_paths
        .contains(&changed.display().to_string()));
    assert!(!original.exists());
    assert!(!changed.exists());
}

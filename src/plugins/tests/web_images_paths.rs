use crate::{config::AppConfig, paths::SaiPaths, plugins, tools};

/// 【搜图路径测试】【短目录注册】用真实临时目录复现 Windows 短文件名中的波浪号。
/// @returns 无；默认图片目录有效，两个工具表均成功注册全部插件
#[test]
fn short_named_picture_paths_register_in_both_registries() {
    let root = tempfile::Builder::new()
        .prefix("RUNNER~1-")
        .tempdir()
        .unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let common = tools::builtin_registry_without_mcp(&config, &paths);
    let readonly = tools::readonly_registry(&config, &paths);
    for registry in [&common, &readonly] {
        assert!(
            registry.plugin_diagnostics().is_empty(),
            "{:?}",
            registry.plugin_diagnostics()
        );
        assert_eq!(
            registry.plugin_owner("search_web_images"),
            Some("web-images")
        );
    }
    let found = plugins::discovery::find(&config, &paths, "web-images").unwrap();
    assert_eq!(
        found.settings()["cache_dir"],
        serde_json::json!(paths.pictures_dir.join("web-images"))
    );
}

/// 【搜图路径测试】【显式目录】含波浪号的缓存目录保留原值，后续目录变更不能继承旧授权。
/// @returns 无；设置与授权精确匹配，管理操作不创建缓存目录
#[test]
fn explicit_tilde_cache_paths_keep_grants_bound_to_the_original_directory() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let original = root.path().join("cache~1");
    plugins::configure(
        &config,
        &paths,
        "web-images",
        serde_json::json!({"cache_dir":original}),
    )
    .unwrap();
    plugins::set_enabled(
        &config,
        &paths,
        "web-images",
        true,
        plugins::GrantUpdate::Declared,
    )
    .unwrap();
    let found = plugins::discovery::find(&config, &paths, "web-images").unwrap();
    assert_eq!(found.settings()["cache_dir"], serde_json::json!(original));
    assert_eq!(
        found
            .capabilities()
            .intersection(&found.grants())
            .binary
            .write_paths,
        [original.to_string_lossy().into_owned()].into()
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
    assert!(found
        .capabilities()
        .intersection(&found.grants())
        .binary
        .write_paths
        .is_empty());
    assert!(!original.exists());
    assert!(!changed.exists());
}

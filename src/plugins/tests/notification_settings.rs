use super::notification_policy::{event, ID};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::config::load_config;
use crate::plugins::{self, GrantUpdate};
use serde_json::json;

/// 【通知设置测试】【字段覆盖】旧设置持续作为缺省值，显式 false 保持有效且不固化其他字段。
#[tokio::test]
async fn notification_settings_override_legacy_fields_without_pinning_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.notification.enabled = false;
    config.notification.sound = false;
    assert!(plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap()
        .notifications
        .is_empty());
    plugins::configure(&config, &paths, ID, json!({"sound":true})).unwrap();
    let plan = plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap();
    assert!(!plan.notifications[0].desktop);
    assert!(plan.notifications[0].sound);
    config.notification.enabled = true;
    assert!(
        plugins::notification_plan(&config, &paths, event())
            .await
            .unwrap()
            .notifications[0]
            .desktop
    );
    plugins::configure(&config, &paths, ID, json!({"enabled":false,"sound":false})).unwrap();
    config.notification.sound = true;
    assert!(plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap()
        .notifications
        .is_empty());
    plugins::configure(&config, &paths, ID, json!({"sound":false})).unwrap();
    for enabled in [false, true] {
        plugins::set_enabled(&config, &paths, ID, enabled, GrantUpdate::Keep).unwrap();
        assert_eq!(
            load_config(&paths).unwrap().plugins[ID].settings,
            json!({"sound":false})
        );
    }
    let plan = plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap();
    assert!(plan.notifications[0].desktop);
    assert!(!plan.notifications[0].sound);
}

/// 【通知设置测试】【原子失败】非法类型、null 与未知配置字段不会替换原文件。
#[test]
fn invalid_notification_settings_do_not_mutate_configuration() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    plugins::configure(&config, &paths, ID, json!({"sound":false})).unwrap();
    let file = paths.config_dir.join("plugins.jsonc");
    let original = std::fs::read(&file).unwrap();
    for invalid in [
        json!(null),
        json!(false),
        json!([]),
        json!({"sound":null}),
        json!({"sound":"false"}),
        json!({"enabled":0}),
        json!({"enabled":null}),
        json!({"extra":true}),
    ] {
        assert!(
            plugins::configure(&config, &paths, ID, invalid.clone()).is_err(),
            "{invalid}"
        );
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
}

/// 【通知设置测试】【配置隔离】外部包不能继承主配置中的通知偏好。
#[test]
fn external_packages_do_not_inherit_legacy_notification_settings() {
    let mut config = AppConfig::default();
    config.notification.enabled = false;
    config.notification.sound = false;
    for id in [ID, "own-notification"] {
        let mut plugin = super::support::descriptor(id, "");
        plugin.setting.settings = json!({"own":true});
        plugin.refresh_compatibility(&config).unwrap();
        assert_eq!(plugin.settings(), &json!({"own":true}));
    }
}

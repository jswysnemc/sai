use super::support::{descriptor, write_package};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::{self, GrantChanges, GrantUpdate};
use sai_plugin_runtime::{
    Capabilities, Notification, PresentationRuntime, PresentationSurface, ReplyPresentation,
    ReplyStatus,
};
use serde_json::json;

pub(super) const ID: &str = "reply-notification";

/// 【通知迁移测试】【事件样本】创建固定的中文完成事件。
/// @returns 可交给正式通知入口的事件
pub(super) fn event() -> ReplyPresentation {
    ReplyPresentation {
        surface: PresentationSurface::Tui,
        status: ReplyStatus::Completed,
        locale: "zh-CN".into(),
    }
}

/// 【通知迁移测试】【旧版对照】两个交互面、三种状态、双语和四种开关组合保留原有结果。
#[tokio::test]
async fn notification_lua_preserves_48_existing_surface_status_locale_and_setting_combinations() {
    let package = plugins::bundled::packages()
        .unwrap()
        .into_iter()
        .find(|package| package.manifest.id == ID)
        .unwrap();
    for enabled in [false, true] {
        for sound in [false, true] {
            let runtime = PresentationRuntime::load(
                package.clone(),
                json!({"enabled":enabled,"sound":sound}),
                package.manifest.capabilities.clone(),
            )
            .unwrap();
            for surface in [PresentationSurface::Tui, PresentationSurface::Web] {
                for (status, english, chinese) in [
                    (ReplyStatus::Completed, "Reply complete", "答复已完成"),
                    (ReplyStatus::Interrupted, "Reply interrupted", "答复已中断"),
                    (ReplyStatus::Failed, "Reply failed", "答复失败"),
                ] {
                    for (locale, body) in [("en-US", english), ("zh-CN", chinese)] {
                        let result = runtime
                            .reply_end(&ReplyPresentation {
                                surface,
                                status,
                                locale: locale.into(),
                            })
                            .await
                            .unwrap();
                        let expected = if enabled || sound {
                            vec![Notification {
                                title: "Sai".into(),
                                body: body.into(),
                                desktop: enabled,
                                sound,
                            }]
                        } else {
                            vec![]
                        };
                        assert_eq!(result, expected);
                    }
                }
            }
        }
    }
}

/// 【通知迁移测试】【正式入口】通知包在共用注册表可见，但不增加模型工具。
#[tokio::test]
async fn notification_policy_is_loaded_without_adding_model_tools_and_can_be_disabled() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    let registry = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert!(registry.active_plugins().iter().any(|(id, _)| id == ID));
    assert!(registry
        .plugin_commands()
        .iter()
        .any(|(id, command)| id == ID && command.name == "preview"));
    let plan = plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap();
    assert_eq!(plan.notifications.len(), 1);
    assert!(plan.diagnostics.is_empty());
    plugins::set_enabled(&config, &paths, ID, false, GrantUpdate::Keep).unwrap();
    assert!(plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap()
        .notifications
        .is_empty());
    let disabled = crate::tools::builtin_registry_without_mcp(&config, &paths);
    assert_eq!(registry.tool_infos().len(), disabled.tool_infos().len());
    assert!(!disabled.active_plugins().iter().any(|(id, _)| id == ID));
}

/// 【通知迁移测试】【分项授权】外部包必须明确取得通知授权，其他能力更新不能恢复或撤销它。
#[tokio::test]
async fn external_notification_policies_require_explicit_grants() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    plugins::set_enabled(&config, &paths, ID, false, GrantUpdate::Keep).unwrap();
    let mut plugin = descriptor(
        "custom-notice",
        "sai.on('reply_end', function() return {title='Custom',body='Own policy',desktop=true,sound=false} end)",
    );
    plugin.package.manifest.capabilities.notifications = true;
    let source = root.path().join("source");
    write_package(&source, &plugin);
    plugins::install(&source, &paths, false).unwrap();
    let id = "custom-notice";
    for update in [
        GrantUpdate::Keep,
        GrantUpdate::Changes(GrantChanges {
            http: Some(["https://example.test".into()].into()),
            ..Default::default()
        }),
    ] {
        plugins::set_enabled(&config, &paths, id, true, update).unwrap();
        assert!(plugins::notification_plan(&config, &paths, event())
            .await
            .unwrap()
            .notifications
            .is_empty());
    }
    plugins::set_enabled(
        &config,
        &paths,
        id,
        true,
        GrantUpdate::Changes(GrantChanges {
            notifications: Some(true),
            ..Default::default()
        }),
    )
    .unwrap();
    let plan = plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap();
    assert_eq!(plan.notifications[0].title, "Custom");
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
    assert_eq!(
        plugins::notification_plan(&config, &paths, event())
            .await
            .unwrap()
            .notifications
            .len(),
        1
    );
    plugins::set_enabled(
        &config,
        &paths,
        id,
        true,
        GrantUpdate::Changes(GrantChanges {
            notifications: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap()
        .notifications
        .is_empty());
}

/// 【通知迁移测试】【错误隔离】坏策略不影响其他包，下一次计算不会继承 VM 全局变量。
#[tokio::test]
async fn notification_failures_are_isolated_and_each_plan_uses_fresh_state() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    for (id, source) in [
        ("a-broken", "sai.on('reply_end', function() error('broken policy') end)"),
        ("a-counter", "local n=0; sai.on('reply_end', function() n=n+1; return {title='Counter',body=tostring(n),desktop=true,sound=false} end)"),
    ] {
        let mut plugin = descriptor(id, source);
        plugin.package.manifest.capabilities = Capabilities { notifications: true, ..Default::default() };
        write_package(&paths.config_dir.join("plugins").join(id), &plugin);
        plugins::set_enabled(&config, &paths, id, true, GrantUpdate::Declared).unwrap();
    }
    for _ in 0..2 {
        let plan = plugins::notification_plan(&config, &paths, event())
            .await
            .unwrap();
        assert_eq!(plan.notifications.len(), 2);
        assert_eq!(
            plan.notifications
                .iter()
                .find(|item| item.title == "Counter")
                .unwrap()
                .body,
            "1"
        );
        assert_eq!(plan.diagnostics.len(), 1);
        assert_eq!(plan.diagnostics[0].source, "a-broken");
        assert!(plan.diagnostics[0].error.contains("broken policy"));
    }
}

/// 【通知迁移测试】【撤销默认授权】显式撤销内置通知授权后不能通过默认设置恢复。
#[tokio::test]
async fn bundled_notification_authorization_is_revocable() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let config = AppConfig::default();
    plugins::set_enabled(
        &config,
        &paths,
        ID,
        true,
        GrantUpdate::Changes(GrantChanges {
            notifications: Some(false),
            ..Default::default()
        }),
    )
    .unwrap();
    plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Keep).unwrap();
    assert!(plugins::notification_plan(&config, &paths, event())
        .await
        .unwrap()
        .notifications
        .is_empty());
    plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Declared).unwrap();
    assert_eq!(
        plugins::notification_plan(&config, &paths, event())
            .await
            .unwrap()
            .notifications
            .len(),
        1
    );
}

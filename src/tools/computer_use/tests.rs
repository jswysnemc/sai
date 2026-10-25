use super::*;
use crate::permission::{PermissionProfile, PermissionProfileMode};

/// 【桌面测试】【注册入口】工具分组与渐进加载可发现原生入口，默认不会启动驱动。
#[test]
fn native_tool_is_registered_as_its_own_deferred_group() {
    let mut registry = ToolRegistry::new();
    register(&mut registry, &ComputerUseConfig::default());
    assert_eq!(registry.tool_infos().len(), family::TOOLS.len() + 1);
    assert!(registry.contains(TOOL_NAME));
    assert_eq!(
        crate::tools::groups::group_for_tool(TOOL_NAME),
        "computer_use"
    );
    assert!(crate::tools::progressive::is_deferred_tool(
        TOOL_NAME,
        &[crate::config::DEFERRED_ALL_NON_BASE.to_string()]
    ));
}

/// 【桌面测试】【参数约束】拒绝缺少目标、伪造句柄、混用坐标以及前台投递字段。
#[test]
fn invalid_requests_cannot_reach_native_dispatch() {
    for args in [
        json!({}),
        json!({"action":"run_command"}),
        json!({"action":"get_app_state","app":"42"}),
        json!({"action":"get_app_state","app":"  ","window_id":101}),
        json!({"action":"click","app":"42","window_id":101,"snapshot_id":"test"}),
        json!({"action":"click","app":"42","window_id":101,"snapshot_id":"test","element_index":1}),
        json!({"action":"click","app":"42","window_id":101,"snapshot_id":"test","x":1}),
        json!({"action":"click","app":"42","window_id":101,"snapshot_id":"test","x":1,"y":2,"element_index":"g1:0"}),
        json!({"action":"click","app":"42","window_id":101,"snapshot_id":"test","x":1,"y":2,"delivery_mode":"foreground"}),
        json!({"action":"status","driver_path":"arbitrary-command"}),
        json!({"action":"scroll","app":"42","window_id":101,"snapshot_id":"test","direction":"down","amount":0}),
    ] {
        assert!(schema::validate(&args).is_err(), "{args}");
    }
}

/// 【桌面测试】【权限审核】截图、应用枚举和输入同样受桌面权限约束，仅配置查询为只读。
#[test]
fn observation_does_not_bypass_desktop_approval() {
    let mut registry = ToolRegistry::new();
    register(&mut registry, &ComputerUseConfig::default());
    let root = tempfile::tempdir().unwrap();
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Audited,
        root.path().into(),
        None,
    ));
    assert_eq!(
        registry
            .permission_for_call(TOOL_NAME, r#"{"action":"status"}"#)
            .unwrap(),
        ToolPermission::ReadOnly
    );
    for action in [
        "list_apps",
        "list_windows",
        "get_app_state",
        "click",
        "type_text",
        "reset",
        "unknown",
    ] {
        let args = json!({"action":action}).to_string();
        assert_eq!(
            registry.permission_for_call(TOOL_NAME, &args).unwrap(),
            ToolPermission::Writes
        );
        assert!(registry.requires_permission(TOOL_NAME, &args).unwrap());
    }
}

/// 【桌面测试】【默认关闭】状态查询和禁用错误都不运行配置中的可执行文件。
#[tokio::test]
async fn disabled_tool_only_reports_configuration() {
    let config = ComputerUseConfig {
        driver_path: "/nonexistent/computer-use-fixture".into(),
        ..Default::default()
    };
    let mut registry = ToolRegistry::new();
    register(&mut registry, &config);
    let result = registry
        .call(TOOL_NAME, r#"{"action":"status"}"#)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap()["enabled"],
        false
    );
    let error = registry
        .call(TOOL_NAME, r#"{"action":"list_apps"}"#)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("disabled"), "{error:#}");
    assert!(!error.to_string().contains("spawn"));
}

/// 【桌面测试】【计划模式】即使全局启用，计划模式也不能使用原生观察绕过执行边界。
#[tokio::test]
async fn plan_mode_blocks_native_observation_before_driver_start() {
    let mut registry = ToolRegistry::new();
    register(
        &mut registry,
        &ComputerUseConfig {
            enabled: true,
            driver_path: "/nonexistent/fixture".into(),
            ..Default::default()
        },
    );
    let root = tempfile::tempdir().unwrap();
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Plan,
        root.path().into(),
        None,
    ));
    let error = registry
        .call(TOOL_NAME, r#"{"action":"list_apps"}"#)
        .await
        .unwrap_err();
    assert!(!format!("{error:#}").contains("spawn mcp"), "{error:#}");
    assert!(registry
        .call(TOOL_NAME, r#"{"action":"status"}"#)
        .await
        .is_ok());
}

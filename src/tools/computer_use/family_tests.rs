use super::test_support::*;
use super::*;
use crate::permission::{PermissionProfile, PermissionProfileMode};

/// 【桌面测试】【工具发现】每个分动作入口都在同一延迟工具组，契约可通过严格 Schema 校验。
/// 参数: 无；返回: 无。
#[test]
fn family_is_discoverable_as_one_deferred_group() {
    let mut registry = ToolRegistry::new();
    register(&mut registry, &ComputerUseConfig::default());
    for (name, _, _) in family::TOOLS {
        assert!(registry.contains(name));
        assert!(is_tool(name));
        assert_eq!(crate::tools::groups::group_for_tool(name), "computer_use");
        assert!(crate::tools::progressive::is_deferred_tool(
            name,
            &[crate::config::DEFERRED_ALL_NON_BASE.into()]
        ));
        jsonschema::validator_for(&family::parameters(name)).unwrap();
    }
    assert!(!is_tool("computer_execute_script"));
}

/// 【桌面测试】【帧别名】分动作坐标入口支持 from_frame，但不能覆盖动作或混用两个观察。
/// 参数: 无；返回: 无。
#[test]
fn frame_alias_cannot_override_action_or_snapshot() {
    let args = json!({"app":"42","window_id":101,"from_frame":"current","x":1,"y":2});
    let normalized = family::normalize("computer_click", args.clone()).unwrap();
    assert_eq!(normalized["snapshot_id"], "current");
    assert_eq!(normalized["action"], "click");
    assert!(normalized.get("from_frame").is_none());
    let mut wrong = args.clone();
    wrong["snapshot_id"] = json!("other");
    assert!(family::normalize("computer_click", wrong).is_err());
    let mut wrong = args;
    wrong["action"] = json!("reset");
    assert!(family::normalize("computer_click", wrong).is_err());
}

/// 【桌面测试】【入口兼容】新截图入口、帧点击及原有统一入口共享同一原生连接和快照生命周期。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn named_tools_and_original_tool_share_the_same_session() {
    let (session, driver) = session(vec![
        state(true),
        json!({"structuredContent":{"effect":"clicked"}}),
    ]);
    let mut registry = ToolRegistry::new();
    register_session(
        &mut registry,
        &ComputerUseConfig {
            enabled: true,
            ..Default::default()
        },
        session,
    );
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let observed = registry
        .call_with_progress(
            "computer_screenshot",
            r#"{"app":"42","window_id":101,"scale":0.5}"#,
            sender,
        )
        .await
        .unwrap();
    assert_eq!(observed.model_attachments.len(), 1);
    let mut click = input_args(&observed.content, "click");
    let id = click
        .as_object_mut()
        .unwrap()
        .remove("snapshot_id")
        .unwrap();
    click.as_object_mut().unwrap().remove("action");
    click["from_frame"] = id;
    click["x"] = json!(10);
    click["y"] = json!(5);
    registry
        .call("computer_click", &click.to_string())
        .await
        .unwrap();
    let mut reused = input_args(&observed.content, "click");
    reused["element_index"] = json!("g1:0");
    assert!(registry.call(TOOL_NAME, &reused.to_string()).await.is_err());
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
    assert_eq!(driver.calls.lock().unwrap()[1].1["x"], 20.0);
}

/// 【桌面测试】【权限继承】新入口不能绕过关闭开关或计划模式，状态入口不访问原生驱动。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn named_tools_keep_the_original_enablement_and_permission_boundaries() {
    let mut registry = ToolRegistry::new();
    register(&mut registry, &ComputerUseConfig::default());
    assert!(registry
        .call("computer_status", "{}")
        .await
        .unwrap()
        .contains("\"enabled\":false"));
    assert!(registry
        .call("computer_apps", "{}")
        .await
        .unwrap_err()
        .to_string()
        .contains("disabled"));
    let root = tempfile::tempdir().unwrap();
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Plan,
        root.path().into(),
        None,
    ));
    for (name, _, _) in family::TOOLS {
        assert_eq!(
            registry.permission_for_call(name, "{}").unwrap(),
            if *name == "computer_status" {
                ToolPermission::ReadOnly
            } else {
                ToolPermission::Writes
            }
        );
    }
    assert!(registry.call("computer_apps", "{}").await.is_err());
    assert!(registry.call("computer_status", "{}").await.is_ok());
}

/// 【桌面测试】【子代理边界】去除整个桌面工具族，但不影响父代理与无关工具。
/// 参数: 无；返回: 无。
#[test]
fn subagent_filter_removes_all_native_desktop_entry_points() {
    let mut parent = ToolRegistry::new();
    register(&mut parent, &ComputerUseConfig::default());
    parent.register(ToolSpec::new(
        "fixture_read",
        "fixture",
        crate::tools::empty_parameters(),
        |_| async { Ok("fixture".into()) },
    ));
    let mut child = parent.clone();
    exclude_from_subagent(&mut child);
    assert!(child.contains("fixture_read"));
    assert!(!child.contains(TOOL_NAME));
    for (name, _, _) in family::TOOLS {
        assert!(!child.contains(name));
        assert!(parent.contains(name));
    }
}

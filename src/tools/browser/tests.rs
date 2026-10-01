use super::BROWSER_TOOL_NAMES;
use crate::permission::{PermissionProfile, PermissionProfileMode};
use crate::tools::{ToolPermission, ToolRegistry, ToolSpec};
use serde_json::json;

/// 【浏览器工具测试】【注册表】构造只含统一浏览器工具的注册表。
/// @returns 工具注册表
fn registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    super::register(&mut registry);
    registry
}

/// 【浏览器工具测试】【单一入口】模型目录只有 browser，旧名称不再可调用。
#[test]
fn only_browser_is_registered_and_deferred() {
    let registry = registry();
    let names: Vec<_> = registry
        .tool_infos()
        .into_iter()
        .map(|info| info.name)
        .collect();
    assert_eq!(names, ["browser"]);
    assert_eq!(BROWSER_TOOL_NAMES, ["browser"]);
    assert_eq!(crate::tools::groups::group_for_tool("browser"), "browser");
    assert!(crate::tools::progressive::is_deferred_tool(
        "browser",
        &[crate::config::DEFERRED_ALL_NON_BASE.to_string()]
    ));
    for old in [
        "browser_navigate",
        "browser_click",
        "browser_snapshot",
        "browser_screenshot",
    ] {
        assert!(registry.permission(old).is_err());
    }
}

/// 【浏览器工具测试】【合法参数】所有动作均可通过真实注册 Schema 校验。
#[test]
fn all_actions_accept_their_own_arguments() {
    let registry = registry();
    for args in [
        json!({"action":"navigate","url":"example.com"}),
        json!({"action":"back"}),
        json!({"action":"forward"}),
        json!({"action":"reload"}),
        json!({"action":"tabs"}),
        json!({"action":"new_tab"}),
        json!({"action":"switch_tab","id":"abc"}),
        json!({"action":"close_tab"}),
        json!({"action":"wait","selector":"#ready","seconds":2}),
        json!({"action":"snapshot","interactive_only":true}),
        json!({"action":"screenshot","full_page":true}),
        json!({"action":"click","ref":"e1"}),
        json!({"action":"double_click","ref":"e1"}),
        json!({"action":"right_click","ref":"e1"}),
        json!({"action":"hover","ref":"e1"}),
        json!({"action":"type","ref":"e1","text":"","clear":true,"submit":false}),
        json!({"action":"select_option","ref":"e1","option":"A"}),
        json!({"action":"press_key","key":"Control+A"}),
        json!({"action":"scroll","direction":"right","amount":500}),
        json!({"action":"evaluate","expression":"document.title"}),
    ] {
        registry
            .validate_arguments("browser", &args.to_string())
            .unwrap_or_else(|error| panic!("{args}: {error:#}"));
    }
}

/// 【浏览器工具测试】【非法参数】拒绝缺失动作、未知动作、必填字段缺失和混用动作参数。
#[test]
fn schema_rejects_incomplete_and_cross_action_arguments() {
    let registry = registry();
    for args in [
        json!({}),
        json!({"action":"explode"}),
        json!({"action":"navigate"}),
        json!({"action":"click"}),
        json!({"action":"type","ref":"e1"}),
        json!({"action":"switch_tab"}),
        json!({"action":"select_option","ref":"e1"}),
        json!({"action":"evaluate"}),
        json!({"action":"press_key"}),
        json!({"action":"snapshot","expression":"fetch('/')"}),
        json!({"action":"click","ref":"e1","url":"https://example.com"}),
        json!({"action":"type","ref":"e1","text":"x","path":"/etc"}),
        json!({"action":"click","ref":"not-a-ref"}),
    ] {
        assert!(
            registry
                .validate_arguments("browser", &args.to_string())
                .is_err(),
            "{args}"
        );
    }
}

/// 【浏览器工具测试】【动作权限】同一工具的只读动作保持免写入审核，交互与脚本保持写权限。
#[test]
fn permissions_depend_on_action_without_weakening_writes() {
    let mut registry = registry();
    let workspace = tempfile::tempdir().unwrap();
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Audited,
        workspace.path().to_path_buf(),
        None,
    ));
    assert_eq!(
        registry.permission("browser").unwrap(),
        ToolPermission::Writes
    );
    for action in [
        "navigate",
        "back",
        "forward",
        "reload",
        "tabs",
        "new_tab",
        "switch_tab",
        "close_tab",
        "wait",
        "snapshot",
        "screenshot",
        "scroll",
    ] {
        let args = json!({"action":action}).to_string();
        assert_eq!(
            registry.permission_for_call("browser", &args).unwrap(),
            ToolPermission::ReadOnly
        );
        assert!(!registry.requires_permission("browser", &args).unwrap());
    }
    for action in [
        "click",
        "double_click",
        "right_click",
        "hover",
        "type",
        "select_option",
        "press_key",
        "evaluate",
        "unknown",
    ] {
        let args = json!({"action":action}).to_string();
        assert_eq!(
            registry.permission_for_call("browser", &args).unwrap(),
            ToolPermission::Writes
        );
        assert!(registry.requires_permission("browser", &args).unwrap());
    }
    // 1. 【浏览器工具测试】【兼容权限】其他工具未设置动作权限时沿用静态声明
    registry.register(ToolSpec::new("read_probe", "read", json!({}), |_| async {
        Ok(String::new())
    }));
    assert_eq!(
        registry.permission_for_call("read_probe", "{}").unwrap(),
        ToolPermission::ReadOnly
    );
}

/// 【浏览器工具测试】【执行边界】规划模式仍拦截写入；只读导航执行原有地址策略。
#[tokio::test]
async fn plan_execution_keeps_action_permissions_and_url_policy() {
    let mut registry = registry();
    let workspace = tempfile::tempdir().unwrap();
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Plan,
        workspace.path().to_path_buf(),
        None,
    ));
    let error = registry
        .call("browser", r#"{"action":"click","ref":"e1"}"#)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("Plan mode blocked"), "{error:#}");
    let error = registry
        .call(
            "browser",
            r#"{"action":"navigate","url":"file:///etc/passwd"}"#,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("not allowed"), "{error:#}");
    let error = registry
        .call("browser", r#"{"action":"snapshot","expression":"1"}"#)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid browser action arguments"),
        "{error:#}"
    );
}

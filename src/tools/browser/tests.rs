use super::BROWSER_TOOL_NAMES;
use crate::tools::{ToolPermission, ToolRegistry};

/// 【浏览器工具测试】【注册表】构造只含浏览器工具组的注册表。
/// @returns 注册表
fn registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    super::register(&mut registry);
    registry
}

/// 【浏览器工具测试】【工具清单】注册的工具与常量清单一一对应，且全部归入浏览器分组。
#[test]
fn every_browser_tool_is_registered_and_grouped() {
    let registry = registry();
    let mut names: Vec<String> = registry
        .tool_infos()
        .into_iter()
        .map(|info| info.name)
        .collect();
    let mut expected: Vec<&str> = BROWSER_TOOL_NAMES.to_vec();
    names.sort();
    expected.sort_unstable();
    assert_eq!(names, expected);
    for name in BROWSER_TOOL_NAMES {
        assert_eq!(
            crate::tools::groups::group_for_tool(name),
            "browser",
            "{name}"
        );
        assert!(
            crate::tools::progressive::is_deferred_tool(
                name,
                &[crate::config::DEFERRED_ALL_NON_BASE.to_string()]
            ),
            "{name} should load on demand"
        );
    }
    let meta = crate::tools::groups::group_meta("browser");
    assert_eq!(meta.label_zh, "内置浏览器");
    assert!(meta.model_description.contains("refs"));
}

/// 【浏览器工具测试】【权限分级】改变页面状态的工具需要写权限，读取与导航保持只读。
#[test]
fn page_mutating_tools_require_write_permission() {
    let registry = registry();
    for name in [
        "browser_click",
        "browser_type",
        "browser_select_option",
        "browser_press_key",
        "browser_evaluate",
    ] {
        assert_eq!(
            registry.permission(name).unwrap(),
            ToolPermission::Writes,
            "{name}"
        );
    }
    for name in [
        "browser_navigate",
        "browser_tabs",
        "browser_wait",
        "browser_snapshot",
        "browser_screenshot",
        "browser_scroll",
    ] {
        assert_eq!(
            registry.permission(name).unwrap(),
            ToolPermission::ReadOnly,
            "{name}"
        );
    }
}

/// 【浏览器工具测试】【参数校验】必填字段缺失或出现未知字段时在调用浏览器前被拒绝。
#[test]
fn schemas_reject_missing_and_unknown_arguments() {
    let registry = registry();
    assert!(registry
        .validate_arguments("browser_click", r#"{"ref":"e1"}"#)
        .is_ok());
    assert!(registry.validate_arguments("browser_click", "{}").is_err());
    assert!(registry
        .validate_arguments("browser_type", r#"{"ref":"e1","text":"x","path":"/etc"}"#)
        .is_err());
    assert!(registry
        .validate_arguments("browser_tabs", r#"{"action":"explode"}"#)
        .is_err());
}

/// 【浏览器工具测试】【地址拦截】导航工具拒绝本地文件协议，不会启动浏览器。
#[tokio::test]
async fn navigate_rejects_file_urls_before_launching() {
    let registry = registry();
    let error = registry
        .call("browser_navigate", r#"{"url":"file:///etc/passwd"}"#)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("not allowed"), "{error:#}");
}

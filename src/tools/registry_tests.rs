use super::*;
use crate::permission::{PermissionProfile, PermissionProfileMode};
use std::path::PathBuf;
use std::sync::Mutex;

/// 验证批准后的网络命令不会再注入沙箱标记。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[tokio::test]
async fn approved_network_command_reaches_handler_without_sandbox_marker() {
    let received = Arc::new(Mutex::new(None));
    let handler_received = Arc::clone(&received);
    let mut registry = ToolRegistry::new();
    registry.register(
        ToolSpec::new(
            "run_command",
            "test",
            empty_parameters(),
            move |arguments| {
                let handler_received = Arc::clone(&handler_received);
                async move {
                    *handler_received.lock().unwrap() = Some(arguments);
                    Ok("ok".to_string())
                }
            },
        )
        .writes(),
    );
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Audited,
        PathBuf::from("/workspace/project"),
        None,
    ));
    let arguments = r#"{"command":"curl https://example.com"}"#;

    registry
        .record_permission_approved("run_command", arguments, None)
        .unwrap();
    registry.call("run_command", arguments).await.unwrap();

    let received = received.lock().unwrap();
    assert!(received
        .as_ref()
        .is_some_and(|arguments| arguments.get("_sai_sandbox").is_none()));
}

/// 验证协议别名可以解析到本地注册工具。
#[test]
fn resolves_accepts_protocol_aliases() {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "str_replace",
        "test",
        empty_parameters(),
        |_arguments| async move { Ok(String::new()) },
    ));

    assert!(registry.resolves("str_replace"));
    assert!(registry.resolves("replace_file_lines"));
    assert!(!registry.resolves("nonexistent_tool"));
}

/// 验证工具定义按首次注册顺序输出，替换工具不会改变既有前缀。
#[test]
fn definitions_preserve_registration_order() {
    let mut registry = ToolRegistry::new();
    for name in ["beta", "alpha", "gamma"] {
        registry.register(ToolSpec::new(
            name,
            "test",
            empty_parameters(),
            |_arguments| async move { Ok(String::new()) },
        ));
    }
    registry.register(ToolSpec::new(
        "beta",
        "updated",
        empty_parameters(),
        |_arguments| async move { Ok(String::new()) },
    ));

    let names = registry
        .definitions()
        .into_iter()
        .map(|definition| definition.function.name)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["beta", "alpha", "gamma"]);
}

/// 验证过滤复制沿用来源注册顺序，而不是沿用白名单顺序。
#[test]
fn clone_filtered_preserves_source_order() {
    let mut registry = ToolRegistry::new();
    for name in ["first", "second", "third"] {
        registry.register(ToolSpec::new(
            name,
            "test",
            empty_parameters(),
            |_arguments| async move { Ok(String::new()) },
        ));
    }

    let filtered = registry.clone_filtered(&["third", "first"]);
    let names = filtered
        .definitions()
        .into_iter()
        .map(|definition| definition.function.name)
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["first", "third"]);
}

/// 验证参数校验区分合法对象、畸形 JSON 和非对象值。
#[test]
fn check_arguments_rejects_malformed_and_non_object_values() {
    let registry = ToolRegistry::new();

    assert!(registry.check_arguments("{}").is_ok());
    assert!(registry.check_arguments("").is_ok());
    assert!(registry.check_arguments(r#"{"path":"/tmp/a"}"#).is_ok());

    let truncated = registry.check_arguments(r#"{"path":"/tmp/a"#).unwrap_err();
    assert!(!truncated.to_string().is_empty());

    let array = registry.check_arguments("[1,2]").unwrap_err();
    assert!(array.to_string().contains("got array"));

    let scalar = registry.check_arguments("42").unwrap_err();
    assert!(scalar.to_string().contains("got number"));
}

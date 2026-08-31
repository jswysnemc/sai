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

/// 验证 resolves 判断工具是否可解析。
#[test]
fn resolves_accepts_registered_tools() {
    let mut registry = ToolRegistry::new();
    registry.register(ToolSpec::new(
        "str_replace",
        "test",
        empty_parameters(),
        |_arguments| async move { Ok(String::new()) },
    ));

    assert!(registry.resolves("str_replace"));
    assert!(!registry.resolves("nonexistent_tool"));
}

/// dsh bash 的执行别名复用 run_command handler，但只在授权后注入本地 Shell。
#[tokio::test]
async fn dsh_bash_alias_uses_run_command_with_private_shell_override() {
    let received = Arc::new(Mutex::new(None));
    let handler_received = Arc::clone(&received);
    let mut registry = ToolRegistry::new();
    registry.register(
        ToolSpec::new(
            "run_command",
            "test",
            json!({
                "type": "object",
                "properties": {"command": {"type": "string"}},
                "required": ["command"],
                "additionalProperties": false
            }),
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

    registry
        .call(DSH_BASH_EXECUTION_ALIAS, r#"{"command":"printf hello"}"#)
        .await
        .unwrap();

    let received = received.lock().unwrap();
    let arguments = received.as_ref().unwrap();
    assert_eq!(arguments["command"], "printf hello");
    assert!(arguments["_sai_command_shell"]
        .as_str()
        .is_some_and(|shell| shell.to_ascii_lowercase().contains("bash")));
    assert_eq!(
        registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name)
            .collect::<Vec<_>>(),
        ["run_command"]
    );
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

/// 白名单过滤只收窄工具集合：会话归属与跨会话开关必须随注册表保留。
///
/// Agent 白名单（如"代码 Agent"的 enabled_tools）走 `clone_filtered` 收窄工具，
/// 若这里把会话归属和 `mesh.cross_session` 重置掉，配了 true 也会被权限策略拦下。
#[test]
fn clone_filtered_preserves_session_ownership_and_cross_session() {
    let mut registry = ToolRegistry::new();
    for name in ["mesh_send", "read_file"] {
        registry.register(ToolSpec::new(
            name,
            "test",
            empty_parameters(),
            |_arguments| async move { Ok(String::new()) },
        ));
    }
    registry.set_session_ownership(
        "state/dir".to_string(),
        "session-1".to_string(),
        true,
    );

    let filtered = registry.clone_filtered(&["mesh_send"]);
    assert_eq!(filtered.session_key, "state/dir");
    assert_eq!(filtered.session_id, "session-1");
    assert!(
        filtered.mesh_cross_session,
        "cross_session 开关必须在白名单过滤后保留"
    );
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

/// 空参数按空对象处理。
#[test]
fn empty_arguments_become_an_empty_object() {
    assert_eq!(parse_arguments("   ").unwrap(), json!({}));
}

/// 严格有效的 JSON 按原样解析。
#[test]
fn strict_json_arguments_parse_unchanged() {
    assert_eq!(
        parse_arguments(r#"{"command":"echo probe-again"}"#).unwrap(),
        json!({"command": "echo probe-again"})
    );
}

/// 参数后面多带一段内容时取第一个完整 JSON，而不是整次调用失败。
///
/// 模型流式吐参数时会在有效 JSON 后残留另一个 JSON 或说明文字；严格解析
/// 报 "trailing characters" 会让工具调用失败并拖垮正在跑的子代理。
#[test]
fn trailing_content_after_arguments_still_parses() {
    assert_eq!(
        parse_arguments(r#"{"command":"echo probe-again"}{"command":"rm -rf /"}"#).unwrap(),
        json!({"command": "echo probe-again"})
    );
    assert_eq!(
        parse_arguments("{\"path\":\"a\"}\n这里还有一段解释").unwrap(),
        json!({"path": "a"})
    );
}

/// 嵌套对象与数组内的尾随内容不影响外层解析。
#[test]
fn nested_arguments_with_trailing_text_parse() {
    let arguments = r#"{"files":[{"path":"a.rs"},{"path":"b.rs"}],"note":"x"} trailing"#;
    assert_eq!(
        parse_arguments(arguments).unwrap(),
        json!({"files": [{"path": "a.rs"}, {"path": "b.rs"}], "note": "x"})
    );
}

/// 完全不是 JSON 时仍然报错，不能因为容错而静默吞掉坏输入。
#[test]
fn non_json_arguments_still_fail() {
    assert!(parse_arguments("这不是 JSON").is_err());
    assert!(parse_arguments("{broken").is_err());
}

/// 容错只接受对象：工具参数在契约上是对象，尾随内容后取出标量或数组时
/// 继续放行等于让 `run_command` 这类工具拿着无意义入参执行。
#[test]
fn trailing_content_after_a_non_object_argument_still_fails() {
    let array = parse_arguments("[\"a\",\"b\"] 后面还有内容").unwrap_err();
    assert!(array.to_string().contains("tool arguments are not valid JSON"), "{array}");

    let scalar = parse_arguments("123 后面还有内容").unwrap_err();
    assert!(scalar.to_string().contains("tool arguments are not valid JSON"), "{scalar}");

    assert!(parse_arguments("\"just a string\" 后面还有内容").is_err());
}

/// 说明文字在参数之前时不前向扫描：那会把示例对象当成真实参数执行。
#[test]
fn leading_prose_before_arguments_still_fails() {
    let error = parse_arguments("可以这样调用：\n{\"path\":\"a.rs\"}").unwrap_err();

    assert!(
        error
            .to_string()
            .contains("tool arguments are not valid JSON"),
        "{error}"
    );
}

/// 复现子代理的真实故障：几百行的长参数后面残留片段时仍取前面的完整对象。
#[test]
fn long_arguments_with_a_trailing_fragment_still_parse() {
    let arguments = format!(
        "{{\"content\":\"{}\",\"path\":\"a.rs\"}} 残余片段",
        "line\\n".repeat(400)
    );

    let parsed = parse_arguments(&arguments).unwrap();

    assert_eq!(parsed["path"], json!("a.rs"));
    assert_eq!(parsed["content"].as_str().unwrap().lines().count(), 400);
}

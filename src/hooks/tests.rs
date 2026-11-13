use super::*;
use serde_json::json;

/// 【Hooks】【测试配置】参数为事件和脚本，返回最小命令钩子
fn hook(event: HookEvent, script: &str) -> HookItem {
    serde_json::from_value(json!({"name":"test","event":event.as_str(),"kind":"command","script":script,"timeout_ms":1000})).unwrap()
}
#[test]
fn event_aliases_are_compatible() {
    assert_eq!(
        HookEvent::parse("pre_tool_use"),
        Some(HookEvent::PreToolUse)
    );
    assert_eq!(
        HookEvent::parse("tool_execution_start"),
        Some(HookEvent::ToolExecutionStart)
    );
    assert_eq!(HookEvent::parse("made_up"), None);
}
#[test]
fn stdin_retains_json_types() {
    let context = HookContext {
        session_id: "s".into(),
        tool_name: Some("read_file".into()),
        ..Default::default()
    }
    .with("tool_input", json!({"path":"a"}))
    .with("stop_hook_active", true);
    let input = protocol::input(HookEvent::PreToolUse, &context);
    assert_eq!(input["tool_input"]["path"], "a");
    assert_eq!(input["stop_hook_active"], true);
}
#[test]
fn mcp_templates_preserve_nested_types_and_reject_missing_fields() {
    let input = json!({"tool_input":{"count":2,"enabled":true}});
    assert_eq!(
        protocol::expand(
            &json!({"count":"${tool_input.count}","values":["${tool_input.enabled}"]}),
            &input
        )
        .unwrap(),
        json!({"count":2,"values":[true]})
    );
    assert!(protocol::expand(&json!("${missing}"), &input).is_err());
}
#[test]
fn matchers_include_aliases_once_and_reject_invalid_regex() {
    assert!(protocol::matches(
        Some("Edit|Write"),
        HookEvent::PreToolUse,
        &json!({"tool_name":"apply_patch"})
    )
    .unwrap());
    assert!(!protocol::matches(
        Some("^Read$"),
        HookEvent::PreToolUse,
        &json!({"tool_name":"run_command"})
    )
    .unwrap());
    assert!(protocol::matches(Some("["), HookEvent::PreToolUse, &json!({})).is_err());
}
#[test]
fn pretool_allow_never_grants_permission_and_posttool_feedback_cannot_undo() {
    let before = decisions::interpret(
        HookEvent::PreToolUse,
        json!({"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":{"path":"a"}}}),
        None,
    );
    assert!(!before.permission_allow);
    assert_eq!(before.updated_input.unwrap()["path"], "a");
    let after = decisions::interpret(
        HookEvent::PostToolUse,
        json!({"decision":"block","reason":"verify output"}),
        None,
    );
    assert_eq!(after.deny.as_deref(), Some("verify output"));
}
#[test]
fn permission_unsupported_fields_do_not_auto_allow() {
    let result = decisions::interpret(
        HookEvent::PermissionRequest,
        json!({"hookSpecificOutput":{"hookEventName":"PermissionRequest","decision":{"behavior":"allow","updatedPermissions":[]}}}),
        None,
    );
    assert!(!result.permission_allow);
}
#[test]
fn stop_false_wins_over_block_and_observers_cannot_control() {
    let result = decisions::interpret(
        HookEvent::Stop,
        json!({"continue":false,"decision":"block","reason":"again"}),
        None,
    );
    assert!(result.stop);
    assert!(result.deny.is_some());
    let observer = decisions::interpret(
        HookEvent::PostModelRequest,
        json!({"continue":false,"decision":"block","reason":"again"}),
        None,
    );
    assert!(!observer.stop);
    assert!(observer.deny.is_none());
}
#[test]
fn shutdown_timeout_is_bounded() {
    assert_eq!(
        execution::timeout(&hook(HookEvent::SessionEnd, ""), HookEvent::SessionEnd).as_secs(),
        1
    );
    let mut hook = hook(HookEvent::Interrupt, "");
    hook.timeout_ms = Some(900_000);
    assert_eq!(execution::timeout(&hook, HookEvent::Interrupt).as_secs(), 3);
}
#[cfg(unix)]
#[tokio::test]
async fn command_stdin_and_exit_two_control_are_supported() {
    let hook=hook(HookEvent::PreToolUse,"python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"hook_event_name\"]==\"PreToolUse\"; print(\"blocked\",file=sys.stderr); sys.exit(2)'");
    let context = HookContext::default();
    let result = dispatch_with_tools(
        &HooksConfig {
            enabled: true,
            items: vec![hook],
        },
        HookEvent::PreToolUse,
        &context,
        None,
    )
    .await;
    assert_eq!(result.deny.as_deref(), Some("blocked"));
}
#[cfg(unix)]
#[tokio::test]
async fn invalid_stdout_and_nonblocking_failure_are_fail_open() {
    for script in ["printf diagnostic", "exit 7"] {
        let result = dispatch_with_tools(
            &HooksConfig {
                enabled: true,
                items: vec![hook(HookEvent::PreToolUse, script)],
            },
            HookEvent::PreToolUse,
            &HookContext::default(),
            None,
        )
        .await;
        assert!(result.deny.is_none());
    }
}

/// 【Hooks】【大输入阻断】脚本不读输入且退出二时，BrokenPipe不能使拒绝失效
#[cfg(unix)]
#[tokio::test]
async fn exit_two_wins_over_closed_stdin_with_large_input() {
    let context = HookContext::default().with("tool_input", json!({"text":"x".repeat(131_072)}));
    let result = dispatch_with_tools(
        &HooksConfig {
            enabled: true,
            items: vec![hook(HookEvent::PreToolUse, "echo blocked >&2; exit 2")],
        },
        HookEvent::PreToolUse,
        &context,
        None,
    )
    .await;
    assert_eq!(result.deny.as_deref(), Some("blocked"));
}

/// 【Hooks】【最终审批】原参数批准不能放行改写参数，最终参数批准后只消费一次
#[cfg(unix)]
#[tokio::test]
async fn rewritten_arguments_require_their_own_approval() {
    use crate::permission::{PermissionProfile, PermissionProfileMode};
    use crate::tools::{ToolPermission, ToolProgress, ToolRegistry, ToolSpec};
    let root = tempfile::tempdir().unwrap();
    let mut registry = ToolRegistry::new();
    let mut tool = ToolSpec::new(
        "write_probe",
        "probe",
        json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}),
        |args| async move { Ok(args["path"].as_str().unwrap().into()) },
    );
    tool.permission = ToolPermission::Writes;
    registry.register(tool);
    registry.set_permission_profile(PermissionProfile::new(
        PermissionProfileMode::Audited,
        root.path().into(),
        None,
    ));
    let original = json!({"path":root.path().join("a").display().to_string()}).to_string();
    let updated = json!({"path":root.path().join("b").display().to_string()});
    let output=json!({"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","updatedInput":updated}}).to_string();
    registry.hooks = HooksConfig {
        enabled: true,
        items: vec![hook(
            HookEvent::PreToolUse,
            &format!("printf '%s' '{}'", output),
        )],
    };
    registry
        .record_permission_approved("write_probe", &original, None)
        .unwrap();
    let call = crate::llm::ToolCall {
        id: "identity".into(),
        kind: "function".into(),
        function: crate::llm::ToolCallFunction {
            name: "write_probe".into(),
            arguments: original,
        },
    };
    let prepared = registry.prepare_hooked_call(&call).await.unwrap();
    assert!(registry
        .execute_prepared(&prepared, ToolProgress::default())
        .await
        .is_err());
    registry
        .record_permission_approved("write_probe", &prepared.call.function.arguments, None)
        .unwrap();
    assert!(registry
        .execute_prepared(&prepared, ToolProgress::default())
        .await
        .unwrap()
        .content
        .ends_with('b'));
    assert!(registry
        .execute_prepared(&prepared, ToolProgress::default())
        .await
        .is_err());
}

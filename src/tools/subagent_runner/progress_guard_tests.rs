use super::*;
use serde_json::json;

/// 【子任务】【等待测试】真正等待运行中任务不会被快速轮询保护截断；无参数、无返回值。
#[test]
fn blocking_waits_remain_available_until_the_background_task_finishes() {
    let mut guard = ProgressGuard::default();
    let args = r#"{"action":"wait","task_id":"build"}"#;
    let waiting =
        json!({"ok":true,"completed":false,"waited":true,"task":{"status":"running"}}).to_string();
    for _ in 0..8 {
        guard.begin_round();
        assert_eq!(
            guard.observe("background_command", args),
            RepeatVerdict::Allow
        );
        guard.record(
            "background_command",
            args,
            true,
            &waiting,
            Duration::from_secs(2),
        );
        assert!(!guard.finish_round());
    }
    let completed =
        json!({"ok":true,"completed":true,"waited":true,"task":{"status":"exited"}}).to_string();
    for index in 0..4 {
        guard.begin_round();
        guard.record("background_command", args, true, &completed, Duration::ZERO);
        assert_eq!(guard.finish_round(), index == 3);
    }
}

/// 【子任务】【等待测试】瞬间返回的等待不能伪装成长时间阻塞；无参数、无返回值。
#[test]
fn immediate_wait_responses_are_still_checked_for_loops() {
    let mut guard = ProgressGuard::default();
    for index in 0..4 {
        guard.begin_round();
        guard.record(
            "background_command",
            r#"{"action":"wait","task_id":"build"}"#,
            true,
            r#"{"ok":true,"completed":false,"waited":true,"task":{"status":"running"}}"#,
            Duration::ZERO,
        );
        assert_eq!(guard.finish_round(), index == 3);
    }
}

/// 【子任务】【循环测试】参数顺序不影响判断，命令内部空白与等待设置仍有语义；无参数、无返回值。
#[test]
fn argument_keys_ignore_only_command_justification() {
    assert_eq!(
        call_key(
            "run_command",
            r#"{"command":"cat PKGBUILD","justification":"first"}"#
        ),
        call_key(
            "run_command",
            r#"{"justification":"retry","command":"cat PKGBUILD"} trailing"#
        )
    );
    assert_ne!(
        call_key("run_command", r#"{"command":"echo 'a b'"}"#),
        call_key("run_command", r#"{"command":"echo 'ab'"}"#)
    );
    assert_ne!(
        call_key("run_command", r#"{"command":"build","timeout_seconds":10}"#),
        call_key("run_command", r#"{"command":"build","timeout_seconds":60}"#)
    );
    assert_ne!(
        call_key("custom_tool", r#"{"justification":"one"}"#),
        call_key("custom_tool", r#"{"justification":"two"}"#)
    );
}

/// 【子任务】【命令状态测试】后台运行成功启动与结束失败保持不同语义；无参数、无返回值。
#[test]
fn distinguishes_background_launch_from_failed_completion() {
    assert!(tool_succeeded(
        "run_command",
        r#"{"mode":"background","ok":true,"task":{"exit_code":null}}"#
    ));
    assert!(!tool_succeeded(
        "background_command",
        r#"{"ok":true,"completed":true,"task":{"exit_code":2}}"#
    ));
}

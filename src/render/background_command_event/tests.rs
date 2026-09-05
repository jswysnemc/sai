use super::*;
use serde_json::json;

/// 后台动作使用明确的进行时与完成时，避免内部动作名直接拼接。
#[test]
fn call_label_describes_background_actions() {
    for (action, pending, done) in [
        (
            "start",
            "Starting background command",
            "Started background command",
        ),
        (
            "list",
            "Checking background commands",
            "Checked background commands",
        ),
        (
            "output",
            "Reading background output",
            "Read background output",
        ),
        (
            "wait",
            "Waiting for background command",
            "Waited for background command",
        ),
        (
            "stop",
            "Stopping background command",
            "Stopped background command",
        ),
        (
            "cleanup",
            "Clearing finished background commands",
            "Cleared finished background commands",
        ),
    ] {
        let args = json!({"action": action}).to_string();
        assert_eq!(background_command_call_label(Some(&args)), pending);
        assert_eq!(
            background_command_call_label_tense(Some(&args), ToolVerbTense::Perfect),
            done
        );
    }
}

/// 启动结果说明命令进入后台，保留任务身份与进程号。
#[test]
fn start_result_summarizes_task_identity() {
    let output = json!({"task": {
        "id": "task-123", "label": "dev server", "pid": 12345, "timeout_seconds": 0
    }})
    .to_string();
    assert_eq!(
        background_command_result_label(&output).unwrap(),
        "Started dev server in background · task-123 · PID 12345"
    );
}

/// 列表、输出和清理结果采用数量说明，移除键值式文案。
#[test]
fn list_output_and_cleanup_results_are_compact() {
    let cases = [
        (
            json!({"tasks": [{"status": "running"}, {"status": "exited"}, {"status": "timed_out"}]}),
            "Background commands · 1 running · 1 finished · 0 stopped · 1 timed out",
        ),
        (
            json!({"task": {"id": "task-1"}, "stdout": "one\ntwo", "stderr": ""}),
            "Read background output task-1 · 2 stdout lines · 0 stderr lines",
        ),
        (
            json!({"removed": ["a", "b"], "remaining": 1}),
            "Cleared 2 finished background commands · 1 remaining",
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(
            background_command_result_label(&value.to_string()).unwrap(),
            expected
        );
    }
}

/// 等待超时不会误报命令完成，重复停止使用可读的终态文案。
#[test]
fn waiting_and_stopping_preserve_task_state() {
    let wait =
        json!({"waited": true, "timeout": true, "task": {"id": "task-1", "status": "running"}});
    assert_eq!(
        background_command_result_label(&wait.to_string()).unwrap(),
        "Stopped waiting for background command task-1 · still running"
    );
    let stop = json!({"was_running": true, "task": {"id": "task-1", "status": "stopped"}});
    assert_eq!(
        background_command_result_label(&stop.to_string()).unwrap(),
        "Stopped background command task-1"
    );
    let already = json!({"was_running": false, "task": {"id": "task-1", "status": "timed_out"}});
    assert_eq!(
        background_command_result_label(&already.to_string()).unwrap(),
        "Background command task-1 already timed out"
    );
}

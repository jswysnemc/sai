use super::*;
use crate::llm::ToolCallStreamProgress;
use crate::runner::{AutomaticInputEvent, AutomaticInputKind};

/// 验证运行中断事件包含可供前端展开的诊断详情。
#[test]
fn interrupted_event_contains_diagnostic_detail() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);

    let events = assembler.map(RunnerEvent::Interrupted);
    let interrupted = events
        .iter()
        .find(|event| event.kind == "run.interrupted")
        .unwrap();

    assert!(interrupted.payload["detail"]
        .as_str()
        .is_some_and(|detail| !detail.trim().is_empty()));
}

/// 验证权限决定会作为可重放事件发送到 Web 消息流。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn emits_permission_resolution_for_stream_replay() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);

    let events = assembler.map(RunnerEvent::Agent(AgentEvent::PermissionResolved {
        request_id: "permission".to_string(),
        decision: crate::permission::PermissionDecision::Deny {
            reply: Some("保留文件".to_string()),
        },
    }));
    let resolved = events
        .iter()
        .find(|event| event.kind == "permission.resolved")
        .unwrap();

    assert_eq!(resolved.payload["request_id"], "permission");
    assert_eq!(resolved.payload["decision"]["decision"], "deny");
    assert_eq!(resolved.payload["decision"]["reply"], "保留文件");
}

#[test]
fn keeps_tool_progress_and_result_on_one_tool_id() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let preparing = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
        ToolCallStreamProgress {
            edit_diff_counts: None,
            index: 0,
            name: Some("edit_file".to_string()),
            arguments_chars: 10,
            arguments_bytes: 10,
            arguments_preview: "{\"patch\":".to_string(),
        },
    )));
    let started = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
        name: "edit_file".to_string(),
        arguments: "{}".to_string(),
    }));
    let result = assembler.map(RunnerEvent::Agent(AgentEvent::ToolResult {
        name: "edit_file".to_string(),
        ok: true,
        output: "ok".to_string(),
    }));
    let id = preparing
        .iter()
        .find(|event| event.kind == "tool.call.preparing")
        .unwrap()
        .payload["tool_id"]
        .as_str()
        .unwrap();
    assert_eq!(started.last().unwrap().payload["tool_id"], id);
    assert_eq!(result.last().unwrap().payload["tool_id"], id);
}

#[test]
fn hides_internal_command_output_progress_events() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let message = crate::tools::command::encode_command_output_for_test(
        crate::tools::command::CommandOutputStream::Stdout,
        b"building\n",
    );
    let events = assembler.map(RunnerEvent::Agent(AgentEvent::ToolProgress {
        name: "run_command".to_string(),
        message,
    }));

    assert!(events.iter().all(|event| event.kind != "tool.progress"));
}

#[test]
fn does_not_return_to_thinking_after_content() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let first = assembler.map(RunnerEvent::Agent(AgentEvent::Chunk(
        crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "answer".to_string(),
        },
    )));
    let second = assembler.map(RunnerEvent::Agent(AgentEvent::Chunk(
        crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "late".to_string(),
        },
    )));
    assert_eq!(
        first
            .iter()
            .filter(|event| event.kind == "status.changed")
            .map(|event| event.payload["status"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["working"]
    );
    assert!(second
        .iter()
        .filter(|event| event.kind == "status.changed")
        .all(|event| event.payload["status"] != "thinking"));
}

#[test]
fn emits_status_only_when_it_changes() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let first = assembler.map(RunnerEvent::Agent(AgentEvent::Chunk(
        crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "a".to_string(),
        },
    )));
    let second = assembler.map(RunnerEvent::Agent(AgentEvent::Chunk(
        crate::llm::ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "b".to_string(),
        },
    )));
    assert_eq!(
        first
            .iter()
            .filter(|event| event.kind == "status.changed")
            .count(),
        1
    );
    assert_eq!(
        second
            .iter()
            .filter(|event| event.kind == "status.changed")
            .count(),
        0
    );
}

#[test]
fn allocates_new_id_when_provider_index_restarts_next_round() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let first = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
        ToolCallStreamProgress {
            edit_diff_counts: None,
            index: 0,
            name: Some("edit_file".to_string()),
            arguments_chars: 10,
            arguments_bytes: 10,
            arguments_preview: "{}".to_string(),
        },
    )));
    assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
        name: "edit_file".to_string(),
        arguments: "{}".to_string(),
    }));
    assembler.map(RunnerEvent::Agent(AgentEvent::ToolResult {
        name: "edit_file".to_string(),
        ok: true,
        output: "ok".to_string(),
    }));
    let second = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
        ToolCallStreamProgress {
            edit_diff_counts: None,
            index: 0,
            name: Some("read_file".to_string()),
            arguments_chars: 10,
            arguments_bytes: 10,
            arguments_preview: "{}".to_string(),
        },
    )));
    let first_id = first.last().unwrap().payload["tool_id"].as_str().unwrap();
    let second_id = second.last().unwrap().payload["tool_id"].as_str().unwrap();
    assert_ne!(first_id, second_id);
}

#[test]
fn pairs_started_by_name_when_unnamed_call_is_dropped() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    // 1. 幻影调用只出现在流式阶段且没有名称，最终会被供应商丢弃
    assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
        ToolCallStreamProgress {
            edit_diff_counts: None,
            index: 0,
            name: None,
            arguments_chars: 0,
            arguments_bytes: 1,
            arguments_preview: String::new(),
        },
    )));
    let edit_preparing = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
        ToolCallStreamProgress {
            edit_diff_counts: None,
            index: 1,
            name: Some("edit_file".to_string()),
            arguments_chars: 10,
            arguments_bytes: 10,
            arguments_preview: "{\"patch\":".to_string(),
        },
    )));
    let edit_started = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
        name: "edit_file".to_string(),
        arguments: "{}".to_string(),
    }));
    let prepared_id = edit_preparing.last().unwrap().payload["tool_id"]
        .as_str()
        .unwrap();
    let started_id = edit_started.last().unwrap().payload["tool_id"]
        .as_str()
        .unwrap();
    assert_eq!(prepared_id, started_id);
}

#[test]
fn emits_workspace_change_after_successful_edit() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
        name: "edit_file".to_string(),
        arguments: "{}".to_string(),
    }));
    let events = assembler.map(RunnerEvent::Agent(AgentEvent::ToolResult {
        name: "edit_file".to_string(),
        ok: true,
        output: "ok".to_string(),
    }));
    assert!(events.iter().any(|event| event.kind == "workspace.changed"));
}

/// 验证 Goal 等待外部工作时向 Web 暴露独立状态。
#[test]
fn maps_external_waiting_status() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let events = assembler.map(RunnerEvent::WaitingExternal);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "status.changed");
    assert_eq!(events[0].payload["status"], "waiting_external");
}

/// 验证传输层重连向 Web 暴露带尝试次数的 status.changed。
#[test]
fn maps_reconnecting_status_with_attempts() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let events = assembler.map(RunnerEvent::Agent(AgentEvent::Reconnecting {
        attempt: 2,
        max_attempts: 3,
    }));

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "status.changed");
    assert_eq!(events[0].payload["status"], "reconnecting");
    assert_eq!(events[0].payload["attempt"], 2);
    assert_eq!(events[0].payload["max_attempts"], 3);
}

/// 验证新一轮会重置运行状态与工具配对表，避免上一轮残留渗入下一轮。
#[test]
fn begin_run_resets_status_and_tool_pairing() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run-1", "第一轮", &[]);
    assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
        name: "edit_file".to_string(),
        arguments: "{}".to_string(),
    }));
    assembler.map(RunnerEvent::Completed(crate::llm::ChatResult {
        content: "done".to_string(),
        reasoning: None,
        usage: None,
        tool_calls: Vec::new(),
        duration_ms: 0,
        ttft_ms: 0,
    }));

    assembler.begin_run("run-2", "第二轮", &[]);
    let events = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
        ToolCallStreamProgress {
            edit_diff_counts: None,
            index: 0,
            name: Some("edit_file".to_string()),
            arguments_chars: 10,
            arguments_bytes: 10,
            arguments_preview: "{}".to_string(),
        },
    )));

    let preparing = events
        .iter()
        .find(|event| event.kind == "tool.call.preparing")
        .unwrap();
    assert_eq!(preparing.run_id, "run-2");
    assert!(preparing.payload["tool_id"]
        .as_str()
        .unwrap()
        .starts_with("run-2-"));
    // 状态已重置，新一轮首个事件会重新携带 status.changed
    assert!(events.iter().any(|event| event.kind == "status.changed"));
}

/// 验证 run.started 事件携带本轮输入，供后加入的标签页重建用户气泡。
#[test]
fn started_event_carries_run_input() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run-1", "你好", &["data:image/png;base64,AAAA".to_string()]);

    let events = assembler.map(RunnerEvent::Started);
    let started = events
        .iter()
        .find(|event| event.kind == "run.started")
        .unwrap();

    assert_eq!(started.run_id, "run-1");
    assert_eq!(started.payload["input"], "你好");
    assert_eq!(
        started.payload["image_urls"][0],
        "data:image/png;base64,AAAA"
    );
}

/// 验证自动输入事件向 Web 传递展示文本而不是内部提示。
#[test]
fn maps_automatic_input_message() {
    let mut assembler = EventAssembler::new("workspace", "session");
    assembler.begin_run("run", "", &[]);
    let events = assembler.map(RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
        AutomaticInputKind::ExternalCompletion,
        "后台任务已完成".to_string(),
    )));

    let message = events
        .iter()
        .find(|event| event.kind == "message.automatic.input")
        .unwrap();
    assert_eq!(message.payload["kind"], "external_completion");
    assert_eq!(message.payload["content"], "后台任务已完成");
}

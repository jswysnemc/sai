use super::*;

/// 使用独立桥接器翻译一条扩展通知。
///
/// 参数:
/// - `update`: ACP update 对象
///
/// 返回:
/// - 翻译后的事件和运行状态变更
fn bridge(update: Value) -> BridgedUpdate {
    AcpEventBridge::default().bridge_session_update(&serde_json::json!({
        "sessionId": "s1",
        "update": update
    }))
}

/// 内核声明的斜杠命令要同时进入运行状态和用户可读事件。
#[test]
fn maps_available_commands_to_runtime_state() {
    let bridged = bridge(serde_json::json!({
        "sessionUpdate": "available_commands_update",
        "availableCommands": [
            { "name": "review", "description": "review changes" },
            { "name": "compact", "description": "compact context" }
        ]
    }));
    match bridged.events.first() {
        Some(AgentEvent::ToolProgress { message, .. }) => {
            assert!(message.contains("/review"));
            assert!(message.contains("/compact"));
        }
        other => panic!("expected a command list, got {other:?}"),
    }
    assert_eq!(bridged.available_commands.unwrap()[1]["name"], "compact");
}

/// 空命令列表不产生噪音事件，但必须清空旧运行状态。
#[test]
fn preserves_empty_command_list_update() {
    let bridged = bridge(serde_json::json!({
        "sessionUpdate": "available_commands_update",
        "availableCommands": []
    }));

    assert!(bridged.events.is_empty());
    assert_eq!(bridged.available_commands, Some(serde_json::json!([])));
}

/// Codex 压缩元数据必须进入统一压缩卡，不能显示为普通工具调用。
#[test]
fn maps_codex_context_compaction_lifecycle() {
    let started = bridge(serde_json::json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "compact-1",
        "title": "Context compacting",
        "status": "in_progress",
        "_meta": { "contextCompaction": true }
    }));
    assert!(matches!(
        started.events.first(),
        Some(AgentEvent::CompactionStarted { turn_count: 0, .. })
    ));

    let finished = bridge(serde_json::json!({
        "sessionUpdate": "tool_call_update",
        "toolCallId": "compact-1",
        "status": "completed",
        "_meta": { "contextCompaction": true }
    }));
    assert_eq!(finished.compaction_applied, Some(true));
    assert!(matches!(
        finished.events.first(),
        Some(AgentEvent::CompactionFinished { applied: true, .. })
    ));
}

/// Codex 子智能体原生字段必须保留在工具参数中，供 Web 专用视图解析。
#[test]
fn preserves_codex_subagent_activity_metadata() {
    let bridged = bridge(serde_json::json!({
        "sessionUpdate": "tool_call",
        "toolCallId": "subagent-1",
        "title": "Start subagent audit",
        "status": "completed",
        "rawInput": {
            "agentThreadId": "thread-audit",
            "agentPath": "/root/audit",
            "activityKind": "started"
        },
        "_meta": {
            "codex": {
                "subagent": {
                    "threadId": "thread-audit",
                    "path": "/root/audit",
                    "activity": "started"
                }
            }
        }
    }));
    match bridged.events.first() {
        Some(AgentEvent::ToolCallIdentified { arguments, .. }) => {
            let value: Value = serde_json::from_str(arguments).unwrap();
            assert_eq!(value["agentThreadId"], "thread-audit");
            assert_eq!(
                value["_acp"]["meta"]["codex"]["subagent"]["path"],
                "/root/audit"
            );
        }
        other => panic!("expected identified subagent call, got {other:?}"),
    }
}

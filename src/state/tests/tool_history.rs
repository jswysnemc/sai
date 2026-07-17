use super::*;
use crate::llm::{ChatContent, ChatMessage, ToolCall, ToolCallFunction};
use crate::state::request_projection::project_provider_turn_from_messages;

#[test]
fn interrupted_tool_call_is_preserved_in_follow_up_context() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store
        .start_turn("turn_1", "inspect the repository")
        .unwrap();
    store
        .record_tool_call_started(
            "turn_1",
            1,
            "call_1",
            "read_file",
            r#"{"path":"README.md"}"#,
        )
        .unwrap();
    let guard = PendingTurnGuard::new(store.clone(), "turn_1".to_string());

    drop(guard);

    let history = store.project_history(None).unwrap();
    assert_eq!(history.stats.tail_turns, 1);
    assert!(history.messages.iter().any(|message| {
        message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| calls.iter().any(|call| call.id == "call_1"))
    }));
    assert!(history.messages.iter().any(|message| {
        message.role == "tool"
            && message.tool_call_id.as_deref() == Some("call_1")
            && matches!(
                message.content.as_ref(),
                Some(ChatContent::Text(text)) if text.contains("interrupted before a result")
            )
    }));
    assert!(history.messages.iter().any(|message| {
        message.role == "user"
            && matches!(
                message.content.as_ref(),
                Some(ChatContent::Text(text)) if text.contains("<turn_aborted>")
            )
    }));
}

#[test]
fn large_tool_output_reuses_stable_replacement_after_resume() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let raw_output = "raw output ".repeat(2_000);
    let stable_preview = "stable clipped preview";
    {
        let store = StateStore::new(&paths).unwrap();
        store.start_turn("turn_1", "inspect file").unwrap();
        store
            .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
            .unwrap();
        let result_ref = store
            .save_clipped_tool_output_replacement("call_1", &raw_output, stable_preview)
            .unwrap()
            .unwrap();
        store
            .record_tool_result_completed(
                "turn_1",
                "call_1",
                true,
                "fallback preview",
                Some(&result_ref),
                None,
                raw_output.chars().count(),
            )
            .unwrap();
        store.complete_turn("turn_1", "done", None).unwrap();
    }

    let resumed = StateStore::new(&paths).unwrap();
    let history = resumed.project_history(None).unwrap();
    let tool_message = history
        .messages
        .iter()
        .find(|message| message.role == "tool")
        .unwrap();

    assert_eq!(history.stats.tail_turns, 1);
    assert_eq!(tool_message.tool_call_id.as_deref(), Some("call_1"));
    assert!(matches!(
        tool_message.content.as_ref(),
        Some(ChatContent::Text(text)) if text == stable_preview
    ));
    assert!(!history
        .messages
        .iter()
        .any(|message| matches!(message.content.as_ref(), Some(ChatContent::Text(text)) if text.contains("fallback preview") || text.contains(&raw_output))));
    assert_eq!(resumed.tool_history_summary().unwrap().replacement_count, 1);
}

#[test]
fn session_snapshot_rebuilds_resume_visible_state_after_store_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let raw_output = "large command output ".repeat(1_000);
    {
        let store = StateStore::new(&paths).unwrap();
        store.start_turn("turn_1", "inspect logs").unwrap();
        store
            .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
            .unwrap();
        let result_ref = store
            .save_clipped_tool_output_replacement("call_1", &raw_output, "stable log preview")
            .unwrap()
            .unwrap();
        store
            .record_tool_result_completed(
                "turn_1",
                "call_1",
                true,
                "fallback log preview",
                Some(&result_ref),
                None,
                raw_output.chars().count(),
            )
            .unwrap();
        store.complete_turn("turn_1", "done", None).unwrap();
        crate::state::session_memory::repository::upsert_memory(
            &store.conv_db,
            crate::state::session_memory::model::NewSessionMemory {
                session_id: store.session_id().to_string(),
                summary: "operator is resuming log inspection".to_string(),
                last_summarized_turn_id: Some("turn_1".to_string()),
                last_summarized_seq: 1,
                checkpoint_id: None,
                source_turn_count: 1,
                token_estimate: 48,
            },
        )
        .unwrap();
    }

    let resumed = StateStore::new(&paths).unwrap();
    let snapshot = resumed.session_snapshot(10_000).unwrap();
    let history = resumed.project_history(None).unwrap();
    let tool_message = history
        .messages
        .iter()
        .find(|message| message.role == "tool")
        .unwrap();

    assert_eq!(snapshot.session_id, "default");
    assert_eq!(snapshot.turn_count, 1);
    assert_eq!(snapshot.tool_history.call_count, 1);
    assert_eq!(snapshot.tool_history.result_count, 1);
    assert_eq!(snapshot.tool_history.replacement_count, 1);
    let memory = snapshot
        .session_memory
        .expect("session memory survives resume");
    assert_eq!(memory.last_summarized_turn_id.as_deref(), Some("turn_1"));
    assert_eq!(memory.last_summarized_seq, 1);
    assert_eq!(memory.token_estimate, 48);
    assert!(matches!(
        tool_message.content.as_ref(),
        Some(ChatContent::Text(text)) if text == "stable log preview"
    ));
    assert!(!history
        .messages
        .iter()
        .any(|message| matches!(message.content.as_ref(), Some(ChatContent::Text(text)) if text.contains("fallback log preview") || text.contains(&raw_output))));
}

#[test]
fn compaction_prompt_records_missing_tool_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "inspect file").unwrap();
    store
        .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
        .unwrap();
    store
        .record_tool_result_completed(
            "turn_1",
            "call_1",
            true,
            "preview",
            Some("tool-results/call_1.txt"),
            None,
            10_000,
        )
        .unwrap();
    store.complete_turn("turn_1", "done", None).unwrap();
    for index in 2..=3 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "tail").unwrap();
        store.complete_turn(&turn_id, "tail", None).unwrap();
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();

    let prompt = store
        .build_compaction_summary_prompt(&request, 10_000)
        .unwrap();
    let recovery = store.recovery_snapshot().unwrap();

    assert!(prompt.contains("tool-results/call_1.txt"));
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryReplacementMissing
    );
}

#[test]
fn compaction_prompt_records_missing_tool_result_ref_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "inspect file").unwrap();
    store
        .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
        .unwrap();
    store
        .record_tool_result_completed(
            "turn_1",
            "call_1",
            true,
            "preview",
            Some("tool-results/missing.txt"),
            None,
            10_000,
        )
        .unwrap();
    store.complete_turn("turn_1", "done", None).unwrap();
    for index in 2..=3 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "tail").unwrap();
        store.complete_turn(&turn_id, "tail", None).unwrap();
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();

    let prompt = store
        .build_compaction_summary_prompt(&request, 10_000)
        .unwrap();
    let recovery = store.recovery_snapshot().unwrap();

    assert!(prompt.contains("tool-results/missing.txt"));
    assert!(recovery
        .latest
        .as_ref()
        .unwrap()
        .reason
        .contains("完整输出引用文件缺失"));
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryReplacementMissing
    );
}

#[test]
fn compaction_prompt_rejects_over_budget_history() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", &"user ".repeat(1_000)).unwrap();
    store
        .complete_turn("turn_1", &"assistant ".repeat(1_000), None)
        .unwrap();
    for index in 2..=3 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "tail").unwrap();
        store.complete_turn(&turn_id, "tail", None).unwrap();
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();

    let err = store
        .build_compaction_summary_prompt(&request, 500)
        .unwrap_err();

    assert!(format!("{err:#}").contains("tool history summary prompt over budget"));
}

#[test]
fn provider_projection_blocks_missing_tool_result() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let projection = project_provider_turn_from_messages(
        &[
            assistant_tool_call("call_1"),
            ChatMessage::plain("user", "next"),
        ],
        0,
        10_000,
    );

    let err = store
        .enforce_provider_projection(Some("turn_1"), &projection)
        .unwrap_err();
    let recovery = store.recovery_snapshot().unwrap();

    assert!(format!("{err:#}").contains("tool call without result"));
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryMissingResult
    );
}

#[test]
fn provider_projection_blocks_orphan_tool_result() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let projection = project_provider_turn_from_messages(
        &[
            ChatMessage::tool("call_orphan", "orphan"),
            ChatMessage::plain("user", "next"),
        ],
        0,
        10_000,
    );

    store
        .enforce_provider_projection(Some("turn_1"), &projection)
        .unwrap_err();
    let recovery = store.recovery_snapshot().unwrap();

    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryOrphanResult
    );
}

#[test]
fn provider_projection_blocks_duplicate_tool_result() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let projection = project_provider_turn_from_messages(
        &[
            assistant_tool_call("call_1"),
            ChatMessage::tool("call_1", "first"),
            ChatMessage::tool("call_1", "second"),
            ChatMessage::plain("user", "next"),
        ],
        0,
        10_000,
    );

    store
        .enforce_provider_projection(Some("turn_1"), &projection)
        .unwrap_err();
    let recovery = store.recovery_snapshot().unwrap();

    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryDuplicateResult
    );
}

fn assistant_tool_call(call_id: &str) -> ChatMessage {
    ChatMessage::assistant(
        "",
        Some(vec![ToolCall {
            id: call_id.to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
    )
}

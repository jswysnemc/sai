use super::turns::pending_placeholder;
use super::*;
use crate::llm::ChatMessage;

mod compaction_recovery;
mod context_epoch;
mod session_memory;
mod session_runtime;
mod tool_history;

fn test_paths(state_dir: PathBuf) -> SaiPaths {
    SaiPaths {
        config_dir: PathBuf::new(),
        config_file: PathBuf::new(),
        secrets_file: PathBuf::new(),
        skills_dir: PathBuf::new(),
        data_dir: PathBuf::new(),
        cache_dir: PathBuf::new(),
        state_dir,
        pictures_dir: PathBuf::new(),
        fish_hook_file: PathBuf::new(),
        bash_hook_file: PathBuf::new(),
        zsh_hook_file: PathBuf::new(),
        powershell_hook_file: PathBuf::new(),
    }
}

#[test]
fn turn_lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "hello").unwrap();
    let turns = store.load_turns().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, TurnStatus::Running);
    assert_eq!(turns[0].assistant_content, pending_placeholder());

    store.complete_turn("turn_1", "hi there", None).unwrap();
    let turns = store.load_turns().unwrap();
    assert_eq!(turns[0].status, TurnStatus::Completed);
    assert_eq!(turns[0].assistant_content, "hi there");
}

#[test]
/// 验证陈旧运行轮次在没有助手正文时仍会保留中断边界。
fn preserves_stale_running_turn_without_assistant_content() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "old task").unwrap();
    assert!(store.mark_interrupted_turn_if_needed().unwrap());
    let turns = store.load_turns().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, TurnStatus::Interrupted);
    assert_eq!(turns[0].user_content, "old task");
    assert!(!store.mark_interrupted_turn_if_needed().unwrap());
}

#[test]
fn undo_removes_last_turn() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "hello").unwrap();
    store.complete_turn("turn_1", "hi", None).unwrap();
    store.start_turn("turn_2", "bye").unwrap();
    store.complete_turn("turn_2", "goodbye", None).unwrap();

    let outcome = store.undo_last_turn().unwrap();
    assert_eq!(outcome.removed, 1);
    assert_eq!(outcome.prompt.as_deref(), Some("bye"));
    assert_eq!(store.load_turns().unwrap().len(), 1);
}

#[test]
fn context_rollback_removes_only_the_expected_last_turn() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "first").unwrap();
    store.complete_turn("turn_1", "first answer", None).unwrap();
    store.start_turn("turn_2", "retry this").unwrap();
    store
        .record_tool_call_started("turn_2", 1, "call_2", "read_file", "{}")
        .unwrap();
    store.interrupt_turn("turn_2", "", None).unwrap();

    let outcome = store.rollback_last_turn_context("turn_2").unwrap();

    assert_eq!(outcome.removed, 1);
    assert_eq!(outcome.prompt.as_deref(), Some("retry this"));
    assert_eq!(store.load_turns().unwrap().len(), 1);
    assert_eq!(store.tool_history_summary().unwrap().call_count, 0);
}

#[test]
fn context_rollback_rejects_a_stale_turn_id() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "first").unwrap();
    store.complete_turn("turn_1", "answer", None).unwrap();

    let error = store.rollback_last_turn_context("turn-stale").unwrap_err();

    assert!(error.to_string().contains("latest turn changed"));
    assert_eq!(store.load_turns().unwrap().len(), 1);
}

#[test]
/// 验证无助手正文的主动中断仍会保留用户轮次。
fn interruption_without_assistant_content_preserves_user_turn() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "edit me").unwrap();
    let guard = PendingTurnGuard::new(store.clone(), "turn_1".to_string());

    drop(guard);

    let turns = store.load_turns().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, TurnStatus::Interrupted);
    assert_eq!(turns[0].user_content, "edit me");
}

#[test]
/// 验证部分助手正文会以中断状态进入后续上下文。
fn interruption_with_partial_content_preserves_assistant_context() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "question").unwrap();
    let mut guard = PendingTurnGuard::new(store.clone(), "turn_1".to_string());
    guard.append_chunk(crate::llm::ChatStreamKind::Content, "partial answer");

    drop(guard);

    let turns = store.load_turns().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, TurnStatus::Interrupted);
    assert_eq!(turns[0].assistant_content, "partial answer");
    let conversation = store.load_conversation().unwrap();
    assert_eq!(conversation.len(), 2);
    assert_eq!(conversation[1].content, "partial answer");
}

#[test]
fn reset_conversation_clears_loaded_tools() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();

    store
        .save_loaded_tools(&["web_search".to_string(), "web_fetch".to_string()])
        .unwrap();
    assert_eq!(
        store.load_loaded_tools().unwrap(),
        vec!["web_fetch".to_string(), "web_search".to_string()]
    );

    store.reset_conversation().unwrap();

    assert!(store.load_loaded_tools().unwrap().is_empty());
}

#[test]
fn prompt_change_does_not_clear_conversation_state() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.reset_if_prompt_changed("stable prompt").unwrap();
    store.start_turn("turn_1", "hello").unwrap();
    store.complete_turn("turn_1", "hi", None).unwrap();
    store
        .save_loaded_tools(&["web_search".to_string()])
        .unwrap();
    store
        .add_usage(&Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        })
        .unwrap();

    store.reset_if_prompt_changed("updated prompt").unwrap();

    assert_eq!(store.load_turns().unwrap().len(), 1);
    assert_eq!(
        store.load_loaded_tools().unwrap(),
        vec!["web_search".to_string()]
    );
    assert_eq!(store.usage_snapshot().unwrap().total_tokens, 15);
}

#[test]
fn compaction_summary_is_applied_and_injected() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, &"u".repeat(200)).unwrap();
        store
            .complete_turn(&turn_id, &"a".repeat(200), None)
            .unwrap();
    }
    let messages = vec![ChatMessage::plain("user", "x".repeat(8_000))];

    let request = store
        .select_compaction_for_messages(&messages, 2_000, true)
        .unwrap()
        .expect("compaction request");
    store
        .apply_compaction(&request, "## Goal\n- keep context")
        .unwrap();

    let turns = store.load_turns().unwrap();
    let context = store.compaction_summary_context().unwrap().unwrap();

    assert_eq!(turns.len(), 2);
    assert!(context.contains("<conversation-summary>"));
    assert!(context.contains("keep context"));
}

#[test]
fn reset_conversation_clears_compaction_summary() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, &"u".repeat(200)).unwrap();
        store
            .complete_turn(&turn_id, &"a".repeat(200), None)
            .unwrap();
    }
    let messages = vec![ChatMessage::plain("user", "x".repeat(8_000))];
    let request = store
        .select_compaction_for_messages(&messages, 2_000, true)
        .unwrap()
        .expect("compaction request");
    store.apply_compaction(&request, "summary").unwrap();

    store.reset_conversation().unwrap();

    assert!(store.compaction_summary_context().unwrap().is_none());
}

#[test]
fn reset_conversation_clears_checkpoint_projection() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=3 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("user {index}"))
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("assistant {index}"), None)
            .unwrap();
    }
    let request = store.select_manual_compaction(1).unwrap().unwrap();
    store
        .apply_compaction(&request, "checkpoint summary")
        .unwrap();
    assert!(store
        .project_history(None)
        .unwrap()
        .checkpoint_context
        .is_some());

    store.reset_conversation().unwrap();

    let projected = store.project_history(None).unwrap();
    assert!(projected.checkpoint_context.is_none());
    assert!(projected.entries.is_empty());
}

#[test]
fn compaction_uses_current_context_without_provider_usage() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, &"u".repeat(200)).unwrap();
        store
            .complete_turn(&turn_id, &"a".repeat(200), None)
            .unwrap();
    }
    let messages = vec![ChatMessage::plain("user", "x".repeat(8_000))];

    assert!(store
        .select_compaction_for_messages(&messages, 100, false)
        .unwrap()
        .is_some());
}

#[test]
fn projection_estimate_matches_message_estimate() {
    let messages = vec![ChatMessage::plain("user", "x".repeat(120))];

    let projection = request_projection::project_provider_turn_from_messages(&messages, 3, 1_000);

    assert_eq!(
        projection.kind,
        request_projection::ProjectionKind::ProviderTurn
    );
    assert_eq!(
        projection.estimate.message_chars,
        compaction::estimate_chat_messages_tokens(&messages)
    );
    assert_eq!(projection.tool_count, 3);
}

#[test]
fn compaction_selection_matches_projection_entry() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, &"u".repeat(200)).unwrap();
        store
            .complete_turn(&turn_id, &"a".repeat(200), None)
            .unwrap();
    }
    let messages = vec![ChatMessage::plain("user", "x".repeat(8_000))];
    let projection = request_projection::project_provider_turn_from_messages(&messages, 0, 2_000);

    let direct = store
        .select_compaction_for_messages(&messages, 2_000, true)
        .unwrap()
        .expect("direct compaction request");
    let projected = store
        .select_compaction_for_projection(&projection, true)
        .unwrap()
        .expect("projected compaction request");

    assert_eq!(projected.compact_turn_ids, direct.compact_turn_ids);
}

#[test]
fn projected_history_without_checkpoint_returns_raw_turns() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "hello").unwrap();
    store.complete_turn("turn_1", "hi", None).unwrap();

    let projected = store.project_history(None).unwrap();

    assert!(projected.checkpoint_context.is_none());
    assert_eq!(projected.stats.covered_turns, 0);
    assert_eq!(projected.stats.tail_turns, 1);
    assert_eq!(projected.entries.len(), 2);
}

#[test]
fn compaction_writes_checkpoint_before_tail_projection() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("user {index}"))
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("assistant {index}"), None)
            .unwrap();
    }
    let request = store.select_manual_compaction(1).unwrap().unwrap();

    store
        .apply_compaction(&request, "stable checkpoint")
        .unwrap();
    let projected = store.project_history(None).unwrap();
    let turns = store.load_turns().unwrap();

    assert!(projected
        .checkpoint_context
        .unwrap()
        .contains("stable checkpoint"));
    assert_eq!(projected.stats.covered_turns, 2);
    assert_eq!(projected.stats.tail_turns, 2);
    assert_eq!(turns.len(), 2);
}

#[test]
fn repeated_compaction_reports_cumulative_checkpoint_coverage() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=6 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("user {index}"))
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("assistant {index}"), None)
            .unwrap();
    }

    let first = store.select_manual_compaction(3).unwrap().unwrap();
    store.apply_compaction(&first, "first summary").unwrap();
    store.start_turn("turn_7", "user 7").unwrap();
    store.complete_turn("turn_7", "assistant 7", None).unwrap();
    let second = store.select_manual_compaction(1).unwrap().unwrap();
    store.apply_compaction(&second, "second summary").unwrap();

    let snapshot = store.session_snapshot(10_000).unwrap();
    let projected = store.project_history(None).unwrap();

    assert_eq!(snapshot.turn_count, 7);
    assert_eq!(snapshot.checkpoint_count, 2);
    assert_eq!(snapshot.checkpoint_covered_turns, 5);
    assert_eq!(snapshot.tail_turns, 2);
    assert_eq!(projected.stats.checkpoint_count, 2);
    assert_eq!(projected.stats.covered_turns, 5);
    assert_eq!(projected.stats.tail_turns, 2);
}

#[test]
fn stale_turn_recovery_writes_recovery_record() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "old task").unwrap();

    let recovered = store.recover_stale_turns().unwrap();
    let recovery = store.recovery_snapshot().unwrap();
    let turns = store.load_turns().unwrap();

    assert_eq!(recovered, 1);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].status, TurnStatus::Interrupted);
    assert_eq!(recovery.stale_turns_recovered, 1);
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::StaleRunningTurn
    );
}

#[test]
fn legacy_compaction_summary_migrates_to_checkpoint_projection() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    {
        let store = StateStore::new(&paths).unwrap();
        for index in 1..=3 {
            let turn_id = format!("turn_{index}");
            store
                .start_turn(&turn_id, &format!("user {index}"))
                .unwrap();
            store
                .complete_turn(&turn_id, &format!("assistant {index}"), None)
                .unwrap();
        }
        compaction::save_summary(&store.compaction_summary_file(), "legacy summary", 2).unwrap();
    }

    let store = StateStore::new(&paths).unwrap();
    let projected = store.project_history(None).unwrap();
    let turns = store.load_turns().unwrap();

    assert!(projected
        .checkpoint_context
        .as_ref()
        .unwrap()
        .contains("legacy summary"));
    assert_eq!(projected.stats.checkpoint_count, 1);
    assert_eq!(projected.stats.covered_turns, 2);
    assert_eq!(projected.stats.tail_turns, 1);
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].turn_id, "turn_3");
    assert_eq!(projected.entries.len(), 2);
    assert!(projected
        .entries
        .iter()
        .all(|entry| entry.content.contains('3')));
}

#[test]
fn session_summary_projection_matches_existing_snapshot_fields() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=3 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("user {index}"))
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("assistant {index}"), None)
            .unwrap();
    }
    store
        .add_usage(&Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        })
        .unwrap();
    let request = store
        .select_manual_compaction(0)
        .unwrap()
        .expect("compaction request");
    store.apply_compaction(&request, "summary").unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();
    let projection = store.project_session_summary(1_000).unwrap();

    assert_eq!(
        projection.kind,
        request_projection::ProjectionKind::SessionSummary
    );
    assert_eq!(projection.stats.session_id, snapshot.session_id);
    assert_eq!(projection.stats.turn_count, snapshot.turn_count);
    assert!(projection.stats.has_compaction_summary);
    assert_eq!(
        projection.estimate.state_context_chars,
        snapshot.context_chars
    );
    assert_eq!(
        projection.stats.usage.total_tokens,
        snapshot.usage.total_tokens
    );
    assert!(projection.warnings.is_empty());
    assert_eq!(
        projection.stats.compacted_turns,
        snapshot
            .compaction
            .as_ref()
            .map(|summary| summary.compacted_turns)
            .unwrap_or_default()
    );
}

#[test]
fn compaction_skips_when_current_context_is_under_threshold() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, &"u".repeat(200)).unwrap();
        store
            .complete_turn(&turn_id, &"a".repeat(200), None)
            .unwrap();
    }
    let messages = vec![ChatMessage::plain("user", "x".repeat(100))];

    assert!(store
        .select_compaction_for_messages(&messages, 2_000, false)
        .unwrap()
        .is_none());
}

#[test]
fn session_snapshot_includes_usage_and_compaction() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=3 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("user {index}"))
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("assistant {index}"), None)
            .unwrap();
    }
    store
        .add_usage(&Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        })
        .unwrap();
    let request = store
        .select_manual_compaction(0)
        .unwrap()
        .expect("compaction request");
    store.apply_compaction(&request, "summary").unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();

    assert!(!snapshot.session_id.is_empty());
    assert_eq!(snapshot.usage.requests, 1);
    assert_eq!(snapshot.usage.total_tokens, 15);
    assert_eq!(snapshot.context_prompt_tokens, 10);
    assert_eq!(snapshot.context_window_tokens, 1_000);
    assert_eq!(snapshot.context_token_ratio, 0.01);
    assert!(snapshot.compaction.is_some());
}

#[test]
fn session_summary_reports_checkpoint_coverage() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    for index in 1..=4 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("user {index}"))
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("assistant {index}"), None)
            .unwrap();
    }
    let request = store.select_manual_compaction(1).unwrap().unwrap();
    store
        .apply_compaction(&request, "checkpoint summary")
        .unwrap();

    let snapshot = store.session_snapshot(10_000).unwrap();

    assert_eq!(snapshot.turn_count, 4);
    assert_eq!(snapshot.checkpoint_count, 1);
    assert_eq!(snapshot.checkpoint_covered_turns, 2);
    assert_eq!(snapshot.tail_turns, 2);
    assert!(snapshot.latest_checkpoint_at.is_some());
}

#[test]
fn session_snapshot_reports_summary_projection_warnings() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "hello").unwrap();
    store.complete_turn("turn_1", "hi", None).unwrap();

    let snapshot = store.session_snapshot(0).unwrap();

    assert!(snapshot
        .projection_warnings
        .iter()
        .any(|warning| warning.contains("invalid context limit")));
}

#[test]
fn session_summary_projection_handles_large_tool_reports_quickly() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let report = "x".repeat(25_000);
    for index in 1..=200 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "user").unwrap();
        store.complete_turn(&turn_id, "assistant", None).unwrap();
        store
            .append_tool_report_context(&turn_id, "run_command", &report)
            .unwrap();
    }

    let started_at = std::time::Instant::now();
    let projection = store.project_session_summary(10_000_000).unwrap();
    let elapsed = started_at.elapsed();

    assert_eq!(projection.stats.tail_turns, 200);
    assert!(projection.estimate.state_context_chars >= report.len() * 200);
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "session summary projection took {elapsed:?}"
    );
}

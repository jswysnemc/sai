use super::test_paths;
use crate::state::{ContextSourceInput, FailureKind, RecoveryStatus, StateStore};

#[test]
fn session_snapshot_reports_context_epoch() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.reset_if_prompt_changed("stable prompt").unwrap();

    let initial = store.session_snapshot(1_000).unwrap();
    let epoch = initial.context_epoch.expect("context epoch summary");

    assert_eq!(epoch.source_count, 1);
    assert_eq!(epoch.last_change_reason, "initialized");
    assert_eq!(epoch.baseline_hash.len(), 64);

    store.reset_if_prompt_changed("updated prompt").unwrap();
    let updated = store.session_snapshot(1_000).unwrap();
    let updated_epoch = updated
        .context_epoch
        .expect("updated context epoch summary");

    assert_eq!(updated_epoch.source_count, 1);
    assert_eq!(updated_epoch.last_change_reason, "stable_source_changed");
    assert_ne!(updated_epoch.baseline_hash, epoch.baseline_hash);
}

#[test]
fn context_epoch_projection_ignores_loaded_tools_dynamic_context() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();

    let initial = store.context_epoch_projection("stable prompt").unwrap();
    store
        .save_loaded_tools(&["web_search".to_string()])
        .unwrap();
    let after_loaded_tools = store.context_epoch_projection("stable prompt").unwrap();

    assert_eq!(initial.baseline, "stable prompt");
    assert_eq!(after_loaded_tools.baseline, "stable prompt");
    assert_eq!(after_loaded_tools.baseline_hash, initial.baseline_hash);
    assert_eq!(after_loaded_tools.source_count, 1);
}

#[test]
fn context_epoch_blocked_source_reuses_previous_baseline() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let initial = store
        .context_epoch_projection_from_sources(vec![ContextSourceInput::available(
            "system_prompt",
            "stable prompt",
        )])
        .unwrap();

    let blocked = store
        .context_epoch_projection_from_sources(vec![ContextSourceInput::blocked(
            "system_prompt",
            "source temporarily unavailable",
        )])
        .unwrap();
    let summary = store
        .session_snapshot(1_000)
        .unwrap()
        .context_epoch
        .expect("context epoch summary");

    assert_eq!(blocked.baseline, initial.baseline);
    assert_eq!(blocked.baseline_hash, initial.baseline_hash);
    assert_eq!(
        blocked.blocked_source.as_deref(),
        Some("system_prompt: source temporarily unavailable")
    );
    assert_eq!(summary.blocked_source, blocked.blocked_source);
}

#[test]
fn context_epoch_blocked_source_without_baseline_records_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();

    let result = store.context_epoch_projection_from_sources(vec![ContextSourceInput::blocked(
        "system_prompt",
        "source missing",
    )]);
    let recovery = store.recovery_snapshot().unwrap();

    assert!(result.is_err());
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ProjectionInvalid
    );
    assert_eq!(
        recovery.latest.as_ref().unwrap().status,
        RecoveryStatus::Terminal
    );
    assert!(recovery
        .latest
        .as_ref()
        .unwrap()
        .reason
        .contains("source missing"));
}

#[test]
fn context_epoch_duplicate_source_key_records_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();

    let result = store.context_epoch_projection_from_sources(vec![
        ContextSourceInput::available("system_prompt", "stable prompt"),
        ContextSourceInput::available("system_prompt", "duplicate prompt"),
    ]);
    let recovery = store.recovery_snapshot().unwrap();

    assert!(result.is_err());
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ProjectionInvalid
    );
    assert_eq!(
        recovery.latest.as_ref().unwrap().status,
        RecoveryStatus::Terminal
    );
    assert!(recovery
        .latest
        .as_ref()
        .unwrap()
        .reason
        .contains("duplicate Context Epoch source key"));
}

#[test]
fn context_epoch_corrupt_snapshot_records_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.context_epoch_projection("stable prompt").unwrap();
    store
        .corrupt_context_epoch_snapshot_for_test("{not valid json")
        .unwrap();

    let result = store.context_epoch_projection("stable prompt");
    let recovery = store.recovery_snapshot().unwrap();

    assert!(result.is_err());
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ProjectionInvalid
    );
    assert_eq!(
        recovery.latest.as_ref().unwrap().status,
        RecoveryStatus::Terminal
    );
    assert!(recovery
        .latest
        .as_ref()
        .unwrap()
        .reason
        .contains("invalid Context Epoch snapshot"));
}

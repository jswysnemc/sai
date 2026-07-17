use super::*;

#[test]
fn session_memory_schema_is_created() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());

    let store = StateStore::new(&paths).unwrap();

    let conn = rusqlite::Connection::open(store.state_dir.join("conversation.db")).unwrap();
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'session_memory'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(exists, 1);
}

#[test]
fn session_snapshot_includes_session_memory_summary() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    crate::state::session_memory::repository::upsert_memory(
        &store.conv_db,
        crate::state::session_memory::model::NewSessionMemory {
            session_id: store.session_id().to_string(),
            summary: "current task, constraints, and next step".to_string(),
            last_summarized_turn_id: Some("turn_3".to_string()),
            last_summarized_seq: 3,
            checkpoint_id: Some("checkpoint_1".to_string()),
            source_turn_count: 3,
            token_estimate: 128,
        },
    )
    .unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();

    let memory = snapshot.session_memory.expect("session memory summary");
    assert_eq!(memory.last_summarized_seq, 3);
    assert_eq!(memory.last_summarized_turn_id.as_deref(), Some("turn_3"));
    assert_eq!(memory.checkpoint_id.as_deref(), Some("checkpoint_1"));
    assert_eq!(memory.source_turn_count, 3);
    assert_eq!(memory.token_estimate, 128);
    assert!(!memory.is_disabled);
}

#[test]
fn complete_turn_updates_session_memory() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "remember this task").unwrap();
    store
        .complete_turn("turn_1", "remember this answer", None)
        .unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();
    let memory = snapshot.session_memory.expect("session memory summary");

    assert_eq!(memory.last_summarized_turn_id.as_deref(), Some("turn_1"));
    assert_eq!(memory.last_summarized_seq, 1);
    assert_eq!(memory.source_turn_count, 1);
    assert!(memory.token_estimate > 0);
}

#[test]
fn reset_conversation_clears_session_memory() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "remember this task").unwrap();
    store
        .complete_turn("turn_1", "remember this answer", None)
        .unwrap();
    assert!(store
        .session_snapshot(1_000)
        .unwrap()
        .session_memory
        .is_some());

    store.reset_conversation().unwrap();

    assert!(store
        .session_snapshot(1_000)
        .unwrap()
        .session_memory
        .is_none());
}

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
fn complete_turn_does_not_overwrite_or_eagerly_create_session_memory() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "remember this task").unwrap();
    store
        .complete_turn("turn_1", "remember this answer", None)
        .unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();
    assert!(snapshot.session_memory.is_none());
    seed_memory(&store, "model-extracted constraint");
    store.start_turn("turn_2", "next question").unwrap();
    store.complete_turn("turn_2", "next answer", None).unwrap();
    assert_eq!(
        crate::state::session_memory::repository::load_memory(&store.conv_db, store.session_id())
            .unwrap()
            .unwrap()
            .summary,
        "model-extracted constraint"
    );
}

#[test]
fn reset_conversation_clears_session_memory() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "remember this task").unwrap();
    store
        .complete_turn("turn_1", "remember this answer", None)
        .unwrap();
    seed_memory(&store, "remembered constraint");
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

/// 【会话记忆】【测试夹具】写入已有模型摘要；参数为存储及正文，无返回值。
fn seed_memory(store: &StateStore, summary: &str) {
    let turns = store.load_turns().unwrap();
    let last = turns.last().unwrap();
    crate::state::session_memory::repository::upsert_memory(
        &store.conv_db,
        crate::state::session_memory::model::NewSessionMemory {
            session_id: store.session_id().into(),
            summary: summary.into(),
            last_summarized_turn_id: Some(last.turn_id.clone()),
            last_summarized_seq: last.seq,
            checkpoint_id: None,
            source_turn_count: turns.len(),
            token_estimate: 20,
        },
    )
    .unwrap();
}

/// 【会话记忆】【压缩重置回归】压缩成功后只累计之后的 40k 增量。
#[test]
fn successful_compaction_resets_session_memory_token_interval() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("old", &" x".repeat(40_001)).unwrap();
    store.complete_turn("old", "", None).unwrap();
    for id in ["tail", "latest"] {
        store.start_turn(id, "tail").unwrap();
        store.complete_turn(id, "tail", None).unwrap();
    }
    let input = store
        .prepare_session_memory_model_extraction(128_000)
        .unwrap()
        .unwrap();
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    store
        .apply_compaction(&request, "compressed history")
        .unwrap();
    assert!(!store
        .apply_session_memory_model_extraction(&input, "obsolete result")
        .unwrap());
    assert!(store
        .prepare_session_memory_model_extraction(128_000)
        .unwrap()
        .is_none());
    store.start_turn("new", &" x".repeat(40_000)).unwrap();
    store.complete_turn("new", "", None).unwrap();
    assert!(store
        .prepare_session_memory_model_extraction(128_000)
        .unwrap()
        .is_none());
    store.start_turn("extra", " x").unwrap();
    store.complete_turn("extra", "", None).unwrap();
    assert!(store
        .prepare_session_memory_model_extraction(128_000)
        .unwrap()
        .is_some());
}

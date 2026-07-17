use super::*;

#[test]
fn session_snapshot_audits_runtime_sequence_gap() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let current_pid = std::process::id();
    store
        .record_runtime_process(crate::runtime_recovery::NewRuntimeProcessRecord {
            id: "proc_1".to_string(),
            session_id: store.session_id().to_string(),
            owner_kind: crate::runtime_recovery::OwnerKind::Session,
            owner_id: store.session_id().to_string(),
            process_kind: crate::runtime_recovery::ProcessKind::BackgroundCommand,
            command: "sleep 60".to_string(),
            cwd: "/tmp".to_string(),
            pid: Some(i64::from(current_pid)),
            pgid: Some(i64::from(current_pid)),
            status: crate::runtime_recovery::RuntimeProcessStatus::Running,
            last_seq: 0,
        })
        .unwrap();
    store
        .conv_db
        .with_conn(|conn| {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO runtime_process_events (
                    id, process_id, seq, stream, event_kind, payload_ref, payload_preview, created_at
                 ) VALUES (?1, 'proc_1', 1, 'stdout', 'output_read', NULL, 'one', ?3),
                          (?2, 'proc_1', 3, 'stdout', 'output_read', NULL, 'three', ?3)",
                rusqlite::params!["event_1", "event_3", now],
            )?;
            conn.execute(
                "UPDATE runtime_processes SET last_seq = 3 WHERE id = 'proc_1'",
                [],
            )?;
            Ok(())
        })
        .unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();
    let failure = snapshot.runtime_recovery.latest_failure.unwrap();

    assert_eq!(
        failure.kind,
        crate::runtime_recovery::RuntimeRecoveryKind::SequenceGap
    );
    assert_eq!(failure.last_safe_seq, Some(1));
}

#[test]
fn session_snapshot_audits_stale_subagent_owner() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let old_pid = std::process::id().saturating_add(1);
    store
        .record_runtime_process(crate::runtime_recovery::NewRuntimeProcessRecord {
            id: "subagent_1".to_string(),
            session_id: store.session_id().to_string(),
            owner_kind: crate::runtime_recovery::OwnerKind::Subagent,
            owner_id: "subagent_1".to_string(),
            process_kind: crate::runtime_recovery::ProcessKind::Subagent,
            command: "explore".to_string(),
            cwd: "/tmp".to_string(),
            pid: Some(i64::from(old_pid)),
            pgid: None,
            status: crate::runtime_recovery::RuntimeProcessStatus::Running,
            last_seq: 0,
        })
        .unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();
    let failure = snapshot.runtime_recovery.latest_failure.unwrap();

    assert_eq!(snapshot.runtime_recovery.active_process_count, 0);
    assert_eq!(snapshot.runtime_recovery.stale_process_count, 1);
    assert_eq!(
        failure.kind,
        crate::runtime_recovery::RuntimeRecoveryKind::StaleOwner
    );
}

#[test]
fn session_snapshot_audits_dead_runtime_process_owner() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store
        .record_runtime_process(crate::runtime_recovery::NewRuntimeProcessRecord {
            id: "background_command_1".to_string(),
            session_id: store.session_id().to_string(),
            owner_kind: crate::runtime_recovery::OwnerKind::Session,
            owner_id: store.session_id().to_string(),
            process_kind: crate::runtime_recovery::ProcessKind::BackgroundCommand,
            command: "sleep 60".to_string(),
            cwd: "/tmp".to_string(),
            pid: Some(i64::from(u32::MAX)),
            pgid: None,
            status: crate::runtime_recovery::RuntimeProcessStatus::Running,
            last_seq: 0,
        })
        .unwrap();

    let snapshot = store.session_snapshot(1_000).unwrap();
    let failure = snapshot.runtime_recovery.latest_failure.unwrap();

    assert_eq!(snapshot.runtime_recovery.active_process_count, 0);
    assert_eq!(snapshot.runtime_recovery.stale_process_count, 1);
    assert_eq!(
        failure.kind,
        crate::runtime_recovery::RuntimeRecoveryKind::StaleOwner
    );
}

#[test]
fn sessions_have_isolated_conversations() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let default_store = StateStore::new(&paths).unwrap();
    default_store.start_turn("turn_default", "default").unwrap();
    default_store
        .complete_turn("turn_default", "default reply", None)
        .unwrap();

    let session = create_session(&paths, Some("work")).unwrap();
    let work_store = StateStore::new(&paths).unwrap();
    assert!(work_store.load_conversation().unwrap().is_empty());
    work_store.start_turn("turn_work", "work").unwrap();
    work_store
        .complete_turn("turn_work", "work reply", None)
        .unwrap();

    switch_session(&paths, "default").unwrap();
    let default_store = StateStore::new(&paths).unwrap();
    let default_history = default_store.load_conversation().unwrap();

    switch_session(&paths, &session.id).unwrap();
    let work_store = StateStore::new(&paths).unwrap();
    let work_history = work_store.load_conversation().unwrap();

    assert!(default_history
        .iter()
        .any(|entry| entry.content == "default"));
    assert!(!default_history.iter().any(|entry| entry.content == "work"));
    assert!(work_history.iter().any(|entry| entry.content == "work"));
    assert!(!work_history.iter().any(|entry| entry.content == "default"));
}

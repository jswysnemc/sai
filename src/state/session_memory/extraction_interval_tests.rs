use super::*;

/// 【会话记忆】【测试夹具】创建隔离数据库；返回临时目录和数据库。
fn fixture() -> (tempfile::TempDir, ConversationDb) {
    let root = tempfile::tempdir().unwrap();
    let db = ConversationDb::open(root.path()).unwrap();
    (root, db)
}

/// 【会话记忆】【测试轮次】写入指定 token 数的正文；参数为数据库、标识和数量。
fn append_turn(db: &ConversationDb, id: &str, tokens: usize) {
    let text = " x".repeat(tokens);
    assert_eq!(crate::token_estimate::estimate_tokens(&text), tokens);
    db.start_turn(id, &text).unwrap();
    db.complete_turn(id, "", None).unwrap();
}

/// 【会话记忆】【阈值回归】低于或恰好达到 40k 不触发，超过才取得租约。
#[test]
fn interval_requires_strictly_more_than_40000_tokens() {
    let (_root, db) = fixture();
    append_turn(&db, "first", 39_999);
    assert!(claim_extraction(
        &db,
        "default",
        &db.active_branch_turns().unwrap(),
        0,
        EXTRACTION_TOKEN_INTERVAL
    )
    .unwrap()
    .is_none());
    append_turn(&db, "second", 1);
    assert!(claim_extraction(
        &db,
        "default",
        &db.active_branch_turns().unwrap(),
        0,
        EXTRACTION_TOKEN_INTERVAL
    )
    .unwrap()
    .is_none());
    append_turn(&db, "third", 1);
    assert!(claim_extraction(
        &db,
        "default",
        &db.active_branch_turns().unwrap(),
        0,
        EXTRACTION_TOKEN_INTERVAL
    )
    .unwrap()
    .is_some());
}

/// 【会话记忆】【并发回归】未完成的提取不会再次启动；失败释放后可以重试。
#[test]
fn pending_extraction_is_not_duplicated_and_failure_can_retry() {
    let (_root, db) = fixture();
    append_turn(&db, "first", 3);
    let turns = db.active_branch_turns().unwrap();
    let ticket = claim_extraction(&db, "default", &turns, 0, 2)
        .unwrap()
        .unwrap();
    assert!(claim_extraction(&db, "default", &turns, 0, 2)
        .unwrap()
        .is_none());
    assert!(release_ticket(&db, "default", &ticket).unwrap());
    let replacement = claim_extraction(&db, "default", &turns, 0, 2)
        .unwrap()
        .unwrap();
    assert!(!release_ticket(&db, "default", &ticket).unwrap());
    assert!(db
        .with_conn(|conn| owns_ticket(conn, "default", &replacement))
        .unwrap());
}

/// 【会话记忆】【恢复回归】重新打开数据库仍从已保存的边界累计。
#[test]
fn saved_interval_survives_database_reopen() {
    let (root, db) = fixture();
    append_turn(&db, "first", 20);
    let boundary = capture_boundary(&db, "default", &db.active_branch_turns().unwrap()).unwrap();
    db.with_conn(|conn| reset_boundary(conn, "default", &boundary))
        .unwrap();
    drop(db);
    let db = ConversationDb::open(root.path()).unwrap();
    append_turn(&db, "second", 2);
    assert!(
        claim_extraction(&db, "default", &db.active_branch_turns().unwrap(), 0, 2)
            .unwrap()
            .is_none()
    );
    append_turn(&db, "third", 1);
    assert!(
        claim_extraction(&db, "default", &db.active_branch_turns().unwrap(), 0, 2)
            .unwrap()
            .is_some()
    );
}

/// 【会话记忆】【轮内回归】压缩时记录运行中轮次，结束后只统计新增正文。
#[test]
fn running_turn_boundary_counts_only_post_compaction_increment() {
    let (_root, db) = fixture();
    db.start_turn("first", &" x".repeat(20)).unwrap();
    let boundary = capture_boundary(&db, "default", &db.active_branch_turns().unwrap()).unwrap();
    db.with_conn(|conn| reset_boundary(conn, "default", &boundary))
        .unwrap();
    db.complete_turn("first", " x x", None).unwrap();
    assert!(
        claim_extraction(&db, "default", &db.active_branch_turns().unwrap(), 0, 2)
            .unwrap()
            .is_none()
    );
    append_turn(&db, "second", 1);
    assert!(
        claim_extraction(&db, "default", &db.active_branch_turns().unwrap(), 0, 2)
            .unwrap()
            .is_some()
    );
}

/// 【会话记忆】【租约恢复】崩溃遗留租约过期后可重新申请，旧结果失去提交权。
#[test]
fn expired_lease_can_be_replaced() {
    let (_root, db) = fixture();
    append_turn(&db, "first", 3);
    let turns = db.active_branch_turns().unwrap();
    let old = claim_extraction(&db, "default", &turns, 0, 2)
        .unwrap()
        .unwrap();
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE session_memory_extraction_interval SET pending_until = 0",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let new = claim_extraction(&db, "default", &turns, 0, 2)
        .unwrap()
        .unwrap();
    assert_ne!(old.id, new.id);
    assert!(!db
        .with_conn(|conn| owns_ticket(conn, "default", &old))
        .unwrap());
}

/// 【会话记忆】【统计范围】工具参数与可见结果计入间隔，私有推理不计入。
#[test]
fn tool_exchange_tokens_are_counted_without_private_reasoning() {
    let root = tempfile::tempdir().unwrap();
    let store =
        crate::state::StateStore::new(&crate::paths::SaiPaths::for_tests(root.path())).unwrap();
    store.start_turn("first", "").unwrap();
    store
        .record_tool_call_started("first", 1, "call", "read_file", "{}")
        .unwrap();
    store
        .record_tool_result_completed(
            "first",
            "call",
            true,
            crate::state::tool_history::ToolResultOutput {
                result_preview: &" x".repeat(100),
                result_ref: None,
                error: None,
                original_chars: 200,
            },
        )
        .unwrap();
    store
        .complete_turn("first", "", Some(&" private".repeat(1_000)))
        .unwrap();
    let turn = store.load_turns().unwrap().remove(0);
    assert_eq!(
        turn_tokens(&store.conv_db, store.session_id(), &turn).unwrap(),
        100 + crate::token_estimate::estimate_tokens("read_file")
            + crate::token_estimate::estimate_tokens("{}")
    );
}

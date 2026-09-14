use crate::state::turns::ConversationDb;

/// 【会话分支】【深度回归】五千轮分支可完整序列化，且响应不包含递归子节点。
/// @returns 无；无参数
#[test]
fn long_session_index_serializes_without_recursive_nodes() {
    let directory = tempfile::tempdir().unwrap();
    let database = ConversationDb::open(directory.path()).unwrap();
    database.with_conn(|conn| {
        conn.execute_batch(
            "WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM sequence WHERE n < 5000)
             INSERT INTO turns(turn_id, seq, user_content, user_timestamp, assistant_content, status, parent_turn_id)
             SELECT 'turn-' || n, n, 'question', '2026-01-01', 'answer', 'completed',
                    CASE WHEN n = 1 THEN '__root__' ELSE 'turn-' || (n - 1) END FROM sequence;",
        )?;
        Ok(())
    }).unwrap();
    let index = database.session_tree_index().unwrap();
    assert_eq!(database.session_tree_counts().unwrap(), (5000, 0));
    assert_eq!(index.total_turns, 5000);
    assert_eq!(index.active_leaf_id.as_deref(), Some("turn-5000"));
    assert!(index.nodes[0].parent_turn_id.is_none());
    let json = serde_json::to_string(&index).unwrap();
    assert!(!json.contains("\"children\""));
    let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded["nodes"].as_array().unwrap().len(), 5000);
}

/// 【会话分支】【分叉兼容】保留分叉数量、同级排序与根部活动位置。
/// @returns 无；无参数
#[test]
fn index_keeps_branches_and_root_navigation() {
    let directory = tempfile::tempdir().unwrap();
    let database = ConversationDb::open(directory.path()).unwrap();
    database.start_turn("first", "first").unwrap();
    database.start_turn("second", "second").unwrap();
    database.switch_active_leaf("first").unwrap();
    database.start_turn("alternative", "alternative").unwrap();
    let index = database.session_tree_index().unwrap();
    assert_eq!(index.branch_points, 1);
    assert_eq!(database.session_tree_counts().unwrap(), (3, 1));
    assert_eq!(index.nodes[2].parent_turn_id.as_deref(), Some("first"));
    assert_eq!(index.active_leaf_id.as_deref(), Some("alternative"));
    database.switch_to_session_start().unwrap();
    assert!(database
        .session_tree_index()
        .unwrap()
        .active_leaf_id
        .is_none());
}

/// 【会话分支】【旧数据兼容】父轮次缺失时保留节点，并沿用摘要和活动叶子语义。
/// @returns 无；无参数
#[test]
fn index_preserves_orphaned_turns_and_preview_fields() {
    let directory = tempfile::tempdir().unwrap();
    let database = ConversationDb::open(directory.path()).unwrap();
    database
        .start_turn("first", "  第一段\n\t第二段  ")
        .unwrap();
    database.start_turn("orphan", "orphan").unwrap();
    database.start_turn("last", "last").unwrap();
    database
        .with_conn(|conn| {
            conn.execute(
                "UPDATE turns SET parent_turn_id = 'missing' WHERE turn_id = 'orphan'",
                [],
            )?;
            conn.execute(
                "UPDATE turns SET assistant_content = ?1 WHERE turn_id = 'first'",
                ["字".repeat(100)],
            )?;
            Ok(())
        })
        .unwrap();
    let index = database.session_tree_index().unwrap();
    assert_eq!(index.total_turns, 3);
    assert_eq!(index.active_leaf_id.as_deref(), Some("last"));
    assert_eq!(index.nodes[0].user_summary, "第一段 第二段");
    assert_eq!(index.nodes[0].assistant_summary.chars().count(), 80);
    assert!(index.nodes[0].assistant_summary.ends_with('…'));
    assert!(index.nodes[1].parent_turn_id.is_none());
    assert_eq!(index.nodes[2].parent_turn_id.as_deref(), Some("orphan"));
    assert_eq!(database.session_tree_counts().unwrap(), (3, 0));
}

use super::flatten_index;
use crate::state::ConversationDb;

/// 【会话分支】【终端深度】五千轮均可选择，标签长度不随父链深度无界增长。
/// @returns 无；无参数
#[test]
fn long_session_rows_preserve_all_turns_with_bounded_labels() {
    let root = tempfile::tempdir().unwrap();
    let database = ConversationDb::open(root.path()).unwrap();
    database.with_conn(|conn| {
        conn.execute_batch(
            "WITH RECURSIVE sequence(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM sequence WHERE n < 5000)
             INSERT INTO turns(turn_id, seq, user_content, user_timestamp, assistant_content, status, parent_turn_id)
             SELECT 'turn-' || n, n, 'question', '2026-01-01', 'answer', 'completed',
                    CASE WHEN n = 1 THEN '__root__' ELSE 'turn-' || (n - 1) END FROM sequence;",
        )?;
        Ok(())
    }).unwrap();
    let rows = flatten_index(&database.session_tree_index().unwrap());
    assert_eq!(rows.len(), 5001);
    assert!(rows[0].turn_id.is_none());
    assert_eq!(rows.last().unwrap().turn_id.as_deref(), Some("turn-5000"));
    assert!(rows.last().unwrap().label.contains('●'));
    assert!(rows.iter().all(|row| row.label.chars().count() < 100));
}

/// 【会话分支】【连接线兼容】浅层分叉保留连接线与活动标记。
/// @returns 无；无参数
#[test]
fn shallow_branches_keep_connectors_and_active_markers() {
    let root = tempfile::tempdir().unwrap();
    let database = ConversationDb::open(root.path()).unwrap();
    database.start_turn("a", "a").unwrap();
    database.start_turn("b", "b").unwrap();
    database.start_turn("d", "d").unwrap();
    database.switch_active_leaf("a").unwrap();
    database.start_turn("c", "c").unwrap();
    let rows = flatten_index(&database.session_tree_index().unwrap());
    assert_eq!(
        rows.iter()
            .filter_map(|row| row.turn_id.as_deref())
            .collect::<Vec<_>>(),
        ["a", "b", "d", "c"]
    );
    assert!(rows[2].label.contains("├─"));
    assert!(rows[3].label.contains('│'));
    assert!(rows[4].label.contains('●'));
}

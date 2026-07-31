use super::*;
use crate::state::tool_history::schema::create_tool_history_tables;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// 创建初始化完成的临时工具历史数据库。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 保持临时目录存活的句柄和数据库连接
fn test_db() -> (TempDir, ConversationDb) {
    let temp = tempfile::tempdir().unwrap();
    let db = ConversationDb::open(temp.path()).unwrap();
    let conn = db.conn.lock().unwrap();
    create_tool_history_tables(&conn).unwrap();
    drop(conn);
    (temp, db)
}

/// 验证工具调用与结果可以生成正确摘要。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn records_call_and_result_summary() {
    let (_temp, db) = test_db();
    insert_tool_call(
        &db,
        NewToolCallRecord {
            session_id: "default".to_string(),
            turn_id: "turn_1".to_string(),
            seq: 1,
            provider_call_id: "call_1".to_string(),
            tool_name: "read_file".to_string(),
            arguments: "{}".to_string(),
        },
    )
    .unwrap();
    insert_tool_result(
        &db,
        NewToolResultRecord {
            session_id: "default".to_string(),
            turn_id: "turn_1".to_string(),
            provider_call_id: "call_1".to_string(),
            ok: true,
            result_preview: "content".to_string(),
            result_ref: None,
            error: None,
            original_chars: 7,
        },
    )
    .unwrap();

    let summary = summarize_tool_history(&db, "default").unwrap();
    assert_eq!(summary.call_count, 1);
    assert_eq!(summary.result_count, 1);
    assert_eq!(summary.pending_count, 0);
    assert_eq!(summary.error_count, 0);
    assert_eq!(summary.latest_tool_name.as_deref(), Some("read_file"));
    assert_eq!(summary.latest_status, Some(ToolCallStatus::Completed));
}

/// 验证截断输出写入引用文件并记录替换次数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn clipped_output_writes_reference_and_replacement() {
    let (temp, db) = test_db();
    let store = StateStore {
        base_state_dir: PathBuf::new(),
        session_id: "default".to_string(),
        state_dir: temp.path().to_path_buf(),
        conv_db: Arc::new(db),
    };

    let result_ref = store
        .save_clipped_tool_output_replacement("call/1", "full output", "preview")
        .unwrap()
        .expect("result ref");
    store
        .record_tool_result_completed(
            "turn_1",
            "call/1",
            true,
            "preview",
            Some(&result_ref),
            None,
            "full output".chars().count(),
        )
        .unwrap();

    assert!(result_ref.starts_with("tool-results/call_1_"));
    assert!(result_ref.ends_with(".txt"));
    assert_eq!(
        std::fs::read_to_string(temp.path().join(&result_ref)).unwrap(),
        "full output"
    );
    let summary = store.tool_history_summary().unwrap();
    assert_eq!(summary.replacement_count, 1);
    assert_eq!(summary.result_count, 1);
}

/// 验证指定轮次的待完成工具调用会结算为中断结果。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn settles_pending_tool_calls_for_turns() {
    let (_temp, db) = test_db();
    insert_tool_call(
        &db,
        NewToolCallRecord {
            session_id: "default".to_string(),
            turn_id: "turn_1".to_string(),
            seq: 1,
            provider_call_id: "call_1".to_string(),
            tool_name: "read_file".to_string(),
            arguments: "{}".to_string(),
        },
    )
    .unwrap();

    let updated =
        settle_pending_tool_calls_for_turns(&db, "default", &["turn_1".to_string()]).unwrap();
    let summary = summarize_tool_history(&db, "default").unwrap();
    let exchanges = load_tool_exchanges_for_turn(&db, "default", "turn_1").unwrap();

    assert_eq!(updated, 1);
    assert_eq!(summary.pending_count, 0);
    assert_eq!(summary.error_count, 1);
    assert_eq!(summary.latest_status, Some(ToolCallStatus::Interrupted));
    assert_eq!(summary.result_count, 1);
    assert_eq!(
        exchanges[0]
            .result
            .as_ref()
            .and_then(|result| result.error.as_deref()),
        Some("tool call was interrupted before a result was recorded")
    );
}

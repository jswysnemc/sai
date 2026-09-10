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
        plugin_state_root: None,
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

/// 【工具历史测试】【连续写入】实际写入大量调用与结果，不能因记录标识碰撞丢失数据。
/// @returns 无；同时检查同一调用重试时保留原记录并更新正文
#[test]
fn rapid_writes_preserve_distinct_records_and_retry_in_place() {
    let (_temp, db) = test_db();
    // 1. 【工具历史测试】【内存数据库】排除磁盘同步对同毫秒写入密度的影响，沿用正式表结构
    {
        let mut connection = db.conn.lock().unwrap();
        *connection = rusqlite::Connection::open_in_memory().unwrap();
        create_tool_history_tables(&connection).unwrap();
    }
    const RECORDS: usize = 65_536;
    for index in 0..RECORDS {
        insert_pair(&db, index, "initial");
    }
    let first = first_pair(&db);
    // 2. 【工具历史测试】【业务键重试】重复提交更新同一记录，不能改变记录标识或增加数量
    insert_pair(&db, 0, "updated");
    assert_eq!(first_pair(&db), first);
    let summary = summarize_tool_history(&db, "rapid").unwrap();
    assert_eq!(summary.call_count, RECORDS);
    assert_eq!(summary.result_count, RECORDS);
    assert_eq!(summary.pending_count, 0);
    let connection = db.conn.lock().unwrap();
    let updated: String = connection
        .query_row(
            "SELECT result_preview FROM tool_results WHERE session_id='rapid' AND provider_call_id='call-0'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(updated, "updated");
}

/// 【工具历史测试】【完整配对】通过正式数据库入口写入一个调用及其结果。
/// @param db 已初始化数据库；index 为不同供应商调用编号；output 为工具结果
/// @returns 无；失败时保留发生冲突的表和调用位置
fn insert_pair(db: &ConversationDb, index: usize, output: &str) {
    let provider_call_id = format!("call-{index}");
    insert_tool_call(
        db,
        NewToolCallRecord {
            session_id: "rapid".into(),
            turn_id: "turn".into(),
            seq: index,
            provider_call_id: provider_call_id.clone(),
            tool_name: "read_file".into(),
            arguments: "{}".into(),
        },
    )
    .unwrap_or_else(|error| panic!("tool call {index}: {error:#}"));
    insert_tool_result(
        db,
        NewToolResultRecord {
            session_id: "rapid".into(),
            turn_id: "turn".into(),
            provider_call_id,
            ok: true,
            result_preview: output.into(),
            result_ref: None,
            error: None,
            original_chars: output.len(),
        },
    )
    .unwrap_or_else(|error| panic!("tool result {index}: {error:#}"));
}

/// 【工具历史测试】【稳定标识】读取首个供应商调用对应的两个内部记录标识。
/// @param db 已包含首个调用与结果的数据库
/// @returns 工具调用与工具结果的内部标识
fn first_pair(db: &ConversationDb) -> (String, String) {
    db.conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT c.id, r.id FROM tool_calls c JOIN tool_results r
             ON c.session_id=r.session_id AND c.provider_call_id=r.provider_call_id
             WHERE c.session_id='rapid' AND c.provider_call_id='call-0'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

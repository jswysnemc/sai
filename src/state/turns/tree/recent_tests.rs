use super::super::rows::decoded_turn_rows;
use crate::paths::SaiPaths;
use crate::state::StateStore;

/// 【会话历史】【规模样本】构造真实状态库和完整线性历史。
/// @param count 历史轮数
/// @returns 临时目录和绑定到该目录的状态库
fn history_fixture(count: usize) -> (tempfile::TempDir, StateStore) {
    let root = tempfile::tempdir().unwrap();
    let store = StateStore::new(&SaiPaths::for_tests(root.path())).unwrap();
    for index in 1..=count {
        let id = format!("turn-{index}");
        store.start_turn(&id, &format!("question-{index}")).unwrap();
        store
            .complete_turn(&id, &format!("answer-{index}"), None)
            .unwrap();
    }
    (root, store)
}

/// 【会话历史】【限量时间线】只解码所需活动轮次，旧历史与其他分支不参与读取。
/// @returns 无；无参数
#[test]
fn recent_timeline_decodes_only_requested_active_turns() {
    let (_root, store) = history_fixture(64);
    store.switch_active_leaf("turn-30").unwrap();
    store.start_turn("alternative", "other question").unwrap();
    store
        .complete_turn("alternative", "other answer", None)
        .unwrap();
    let before = decoded_turn_rows();
    let turns = store.session_timeline(3).unwrap();
    let decoded = decoded_turn_rows() - before;
    assert_eq!(
        turns
            .iter()
            .map(|turn| turn.turn_id.as_str())
            .collect::<Vec<_>>(),
        ["turn-29", "turn-30", "alternative"]
    );
    assert!(
        decoded <= 3,
        "loading 3 turns decoded {decoded} stored turns"
    );
}

/// 【会话历史】【旧消息入口】消息数限制在读取时生效，并保留末尾消息顺序。
/// @returns 无；无参数
#[test]
fn recent_history_decodes_at_most_the_entry_limit() {
    let (_root, store) = history_fixture(64);
    let before = decoded_turn_rows();
    let entries = store.history(4).unwrap();
    let decoded = decoded_turn_rows() - before;
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.content.as_str())
            .collect::<Vec<_>>(),
        ["question-63", "answer-63", "question-64", "answer-64"]
    );
    assert!(
        decoded <= 4,
        "loading 4 messages decoded {decoded} stored turns"
    );
}

/// 【会话历史】【零数量】空窗口不读取任何旧正文。
/// @returns 无；无参数
#[test]
fn empty_history_windows_do_not_decode_turns() {
    let (_root, store) = history_fixture(3);
    let before = decoded_turn_rows();
    assert!(store.history(0).unwrap().is_empty());
    assert!(store.session_timeline(0).unwrap().is_empty());
    assert_eq!(decoded_turn_rows(), before);
}

/// 【会话历史】【损坏父链】父指针成环或缺失时仍返回可读尾部，且不会重复轮次。
/// @returns 无；无参数
#[test]
fn recent_history_stops_at_cycles_and_missing_parents() {
    let (_root, store) = history_fixture(4);
    store
        .conv_db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE turns SET parent_turn_id = 'turn-4' WHERE turn_id = 'turn-3'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    let turns = store.session_timeline(20).unwrap();
    assert_eq!(
        turns
            .iter()
            .map(|turn| turn.turn_id.as_str())
            .collect::<Vec<_>>(),
        ["turn-3", "turn-4"]
    );
    store
        .conv_db
        .with_conn(|conn| {
            conn.execute(
                "UPDATE turns SET parent_turn_id = 'missing' WHERE turn_id = 'turn-4'",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    assert_eq!(store.session_timeline(20).unwrap().len(), 1);
    store.switch_to_session_start().unwrap();
    assert!(store.session_timeline(20).unwrap().is_empty());
}

/// 【会话历史】【消息数兼容】工具报告与未完成轮次仍按照旧消息入口数量截取。
/// @returns 无；无参数
#[test]
fn recent_entries_keep_tool_reports_and_unanswered_user_messages() {
    let (_root, store) = history_fixture(4);
    store
        .conv_db
        .append_tool_report("turn-4", "report-one")
        .unwrap();
    store
        .conv_db
        .append_tool_report("turn-4", "report-two")
        .unwrap();
    store.start_turn("pending", "unanswered").unwrap();
    let entries = store.history(3).unwrap();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.content.as_str())
            .collect::<Vec<_>>(),
        ["report-one", "report-two", "unanswered"]
    );
}

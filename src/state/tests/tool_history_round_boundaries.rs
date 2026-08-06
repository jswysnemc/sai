use super::*;

/// 在运行中轮次写入一条完整的工具调用与结果。
///
/// @param store 状态仓库
/// @param turn_id 轮次标识
/// @param seq 轮内序号
/// @param round 助手子轮编号
/// @param output 工具结果正文
/// @returns 无返回值
fn insert_running_tool_call(
    store: &StateStore,
    turn_id: &str,
    seq: usize,
    round: usize,
    output: &str,
) {
    let call_id = format!("call_{seq}");
    store
        .record_tool_call_started_with_context(
            turn_id,
            seq,
            round,
            None,
            &call_id,
            "read_file",
            r#"{"path":"a.rs"}"#,
        )
        .unwrap();
    store
        .record_tool_result_completed(turn_id, &call_id, true, output, None, None, output.len())
        .unwrap();
}

/// 验证运行中轮次的工具调用参与压缩，且压缩后不再回放。
#[test]
fn running_turn_tool_calls_are_compacted_and_dropped_from_context() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "重构这个模块").unwrap();
    // 1. 单轮内写入 12 条工具调用，每条一个独立子轮
    let payload = "x".repeat(500);
    for index in 1..=12 {
        insert_running_tool_call(&store, "turn_1", index, index, &payload);
    }
    let before = store
        .project_running_turn_tool_messages("turn_1")
        .unwrap()
        .len();

    // 2. 触发压缩，运行中轮次必须被选中
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let running = request
        .running_turn
        .as_ref()
        .expect("运行中轮次必须参与压缩");
    assert_eq!(running.turn_id, "turn_1");
    assert_eq!(
        running.compacted_calls,
        12 - crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS
    );

    store.apply_compaction(&request, "交接笔记正文").unwrap();
    let after = store
        .project_running_turn_tool_messages("turn_1")
        .unwrap()
        .len();

    // 3. 压缩后回放的消息数必须显著下降
    assert!(
        after < before,
        "压缩后运行轮次消息应减少: before={before}, after={after}"
    );
    assert_eq!(
        after,
        crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS * 2,
        "只保留末尾若干条调用及其结果"
    );
}

/// 验证压缩切点不会切断助手工具调用与工具结果的配对。
#[test]
fn compaction_never_splits_a_tool_call_round() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "并行读取").unwrap();
    // 1. 每个子轮写入 3 条并行调用，覆盖子轮边界切分场景
    let payload = "y".repeat(300);
    let mut seq = 1;
    for round in 1..=5 {
        for _ in 0..3 {
            insert_running_tool_call(&store, "turn_1", seq, round, &payload);
            seq += 1;
        }
    }

    let request = store.select_manual_compaction(0).unwrap().unwrap();
    store.apply_compaction(&request, "笔记").unwrap();
    let messages = store.project_running_turn_tool_messages("turn_1").unwrap();

    // 2. 首条消息必须是助手声明，不能出现孤立的工具结果
    if let Some(first) = messages.first() {
        assert_eq!(first.role, "assistant", "压缩后不得以孤立的工具结果开头");
    }
    // 3. 每条工具结果都必须能在此前的助手消息里找到对应调用
    let mut declared = std::collections::BTreeSet::new();
    for message in &messages {
        if let Some(calls) = message.tool_calls.as_ref() {
            for call in calls {
                declared.insert(call.id.clone());
            }
        }
        if message.role == "tool" {
            let id = message.tool_call_id.as_deref().unwrap_or_default();
            assert!(declared.contains(id), "工具结果 {id} 没有配对的工具调用");
        }
    }
}

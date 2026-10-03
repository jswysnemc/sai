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
            crate::state::tool_history::ToolAssistantContext {
                assistant_round: round,
                assistant_reasoning: None,
            },
            &call_id,
            "read_file",
            r#"{"path":"a.rs"}"#,
        )
        .unwrap();
    store
        .record_tool_result_completed(
            turn_id,
            &call_id,
            true,
            crate::state::tool_history::ToolResultOutput {
                result_preview: output,
                result_ref: None,
                error: None,
                original_chars: output.len(),
            },
        )
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

    // 2. 【上下文】【近期结果保护】切点必须向前对齐到完整子轮，不能丢弃末尾四次调用
    assert_eq!(request.running_turn.as_ref().unwrap().compacted_calls, 9);
    let retained = messages
        .iter()
        .filter_map(|message| message.tool_call_id.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    for id in ["call_12", "call_13", "call_14", "call_15"] {
        assert!(retained.contains(id), "近期结果 {id} 必须保留");
    }

    // 3. 首条消息必须是助手声明，不能出现孤立的工具结果
    if let Some(first) = messages.first() {
        assert_eq!(first.role, "assistant", "压缩后不得以孤立的工具结果开头");
    }
    // 4. 每条工具结果都必须能在此前的助手消息里找到对应调用
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

/// 【上下文】【并行保护】只有一个并行子轮时，不能为了压缩而丢掉近期调用
/// 参数: 无；返回无
#[test]
fn a_single_parallel_round_is_not_partially_compacted() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "并行读取").unwrap();
    for seq in 1..=16 {
        insert_running_tool_call(&store, "turn_1", seq, 1, "source result");
    }
    assert!(store.select_manual_compaction(0).unwrap().is_none());
    let projection = crate::state::request_projection::project_provider_turn_from_messages(
        &[crate::llm::ChatMessage::plain(
            "user",
            "pressure ".repeat(1000),
        )],
        0,
        100,
    );
    assert!(store
        .select_compaction_for_projection(&projection, false)
        .unwrap()
        .is_none());
}

/// 【上下文】【自动门槛】按完整子轮对齐后，新增覆盖仍须达到自动压缩门槛
/// 参数: 无；返回无
#[test]
fn automatic_compaction_rechecks_coverage_after_round_alignment() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "分批并行读取").unwrap();
    let projection = crate::state::request_projection::project_provider_turn_from_messages(
        &[crate::llm::ChatMessage::plain(
            "user",
            "pressure ".repeat(1000),
        )],
        0,
        100,
    );
    for seq in 1..=16 {
        insert_running_tool_call(&store, "turn_1", seq, (seq - 1) / 5 + 1, "source result");
    }
    // 1. 【上下文】【自动门槛】十二条回退到十条后，自动压缩应跳过，强制压缩仍可执行
    assert!(store
        .select_compaction_for_projection(&projection, false)
        .unwrap()
        .is_none());
    let forced = store
        .select_compaction_for_projection(&projection, true)
        .unwrap()
        .unwrap();
    assert_eq!(forced.running_turn.unwrap().compacted_calls, 10);

    // 2. 【上下文】【自动门槛】积累足够完整子轮后，自动压缩正常推进
    for seq in 17..=20 {
        insert_running_tool_call(&store, "turn_1", seq, 4, "source result");
    }
    let automatic = store
        .select_compaction_for_projection(&projection, false)
        .unwrap()
        .unwrap();
    assert_eq!(automatic.running_turn.unwrap().compacted_calls, 15);
}

/// 【上下文】【完整轮次保留】运行子轮不可切分时，已完成轮次仍能正常压缩
/// 参数: 无；返回无
#[test]
fn completed_turn_compaction_survives_an_indivisible_running_round() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("completed", "先前任务").unwrap();
    store.complete_turn("completed", "任务完成", None).unwrap();
    store.start_turn("turn_1", "并行读取").unwrap();
    for seq in 1..=16 {
        insert_running_tool_call(&store, "turn_1", seq, 1, "source result");
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    assert_eq!(request.compact_turn_ids, vec!["completed"]);
    assert!(request.running_turn.is_none());
    store.apply_compaction(&request, "先前任务已完成").unwrap();
    let messages = store.project_running_turn_tool_messages("turn_1").unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        16
    );
}

/// 【上下文】【旧边界兼容】旧 checkpoint 切入并行子轮时保留整组，避免删除未覆盖结果
/// 参数: 无；返回无
#[test]
fn legacy_partial_round_boundary_keeps_the_uncovered_results() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "并行读取").unwrap();
    for seq in 1..=15 {
        insert_running_tool_call(&store, "turn_1", seq, (seq - 1) / 3 + 1, "source result");
    }
    let request = crate::state::compaction::CompactionRequest::new(Vec::new(), None)
        .with_running_turn(Some(crate::state::compaction::RunningTurnCompaction {
            turn_id: "turn_1".into(),
            compacted_calls: 11,
        }));
    store
        .apply_compaction(&request, "旧摘要只覆盖前十一条调用")
        .unwrap();
    let messages = store.project_running_turn_tool_messages("turn_1").unwrap();
    assert!(messages
        .iter()
        .any(|message| message.tool_call_id.as_deref() == Some("call_12")));
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        6
    );
}

/// 【上下文】【再次压缩】追加新子轮后只推进到完整边界，近期结果继续保留
/// 参数: 无；返回无
#[test]
fn repeated_parallel_compaction_advances_complete_rounds_only() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "继续并行读取").unwrap();
    for (start, end, expected) in [(1, 15, 9), (16, 21, 15)] {
        for seq in start..=end {
            insert_running_tool_call(&store, "turn_1", seq, (seq - 1) / 3 + 1, "source result");
        }
        let request = store.select_manual_compaction(0).unwrap().unwrap();
        assert_eq!(
            request.running_turn.as_ref().unwrap().compacted_calls,
            expected
        );
        store.apply_compaction(&request, "已完成旧子轮").unwrap();
        let messages = store.project_running_turn_tool_messages("turn_1").unwrap();
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.role == "tool")
                .count(),
            6
        );
    }
}

/// 【上下文】【组合回归】局部摘要后执行全局压缩，保留近期并行结果并允许回读原文
/// 参数: 无；返回无
#[test]
fn context_block_then_parallel_compaction_retains_recent_results() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "并行检查").unwrap();
    let output = "source entry path=a.rs result=ok\n".repeat(300);
    for seq in 1..=15 {
        insert_running_tool_call(&store, "turn_1", seq, (seq - 1) / 3 + 1, &output);
    }
    store
        .compress_context_block(&crate::state::context_blocks::CompressRequest {
            message_ids: vec!["call_1".into()],
            topic: "旧工具结果".into(),
            summary: "a.rs 检查通过".into(),
            expected_revision: 0,
        })
        .unwrap();
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    assert_eq!(request.running_turn.as_ref().unwrap().compacted_calls, 9);
    store
        .apply_compaction(&request, "已检查前三组源码")
        .unwrap();
    let messages = store.project_running_turn_tool_messages("turn_1").unwrap();
    assert_eq!(
        messages
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        6
    );
    let restored = store.restore_context_message("call_1", 0, 100).unwrap();
    assert!(restored.content.starts_with("source entry path=a.rs"));
}

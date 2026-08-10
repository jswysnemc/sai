use super::*;
use crate::llm::{ChatContent, ChatMessage, ToolCall, ToolCallFunction};
use crate::state::request_projection::project_provider_turn_from_messages;

#[test]
fn interrupted_tool_call_is_preserved_in_follow_up_context() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store
        .start_turn("turn_1", "inspect the repository")
        .unwrap();
    store
        .record_tool_call_started(
            "turn_1",
            1,
            "call_1",
            "read_file",
            r#"{"path":"README.md"}"#,
        )
        .unwrap();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let guard = PendingTurnGuard::new(
        store.clone(),
        "turn_1".to_string(),
        crate::state::PartialTurnSink::new(),
    )
    .with_cancel_flag(cancel);

    drop(guard);

    let history = store.project_history(None).unwrap();
    assert_eq!(history.stats.tail_turns, 1);
    assert!(history.messages.iter().any(|message| {
        message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| calls.iter().any(|call| call.id == "call_1"))
    }));
    assert!(history.messages.iter().any(|message| {
        message.role == "tool"
            && message.tool_call_id.as_deref() == Some("call_1")
            && matches!(
                message.content.as_ref(),
                Some(ChatContent::Text(text)) if text.contains("interrupted before a result")
            )
    }));
    assert!(history.messages.iter().any(|message| {
        message.role == "user"
            && matches!(
                message.content.as_ref(),
                Some(ChatContent::Text(text)) if text.contains("<turn_aborted>")
            )
    }));
}

#[test]
fn large_tool_output_reuses_stable_replacement_after_resume() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let raw_output = "raw output ".repeat(2_000);
    let stable_preview = "stable clipped preview";
    {
        let store = StateStore::new(&paths).unwrap();
        store.start_turn("turn_1", "inspect file").unwrap();
        store
            .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
            .unwrap();
        let result_ref = store
            .save_clipped_tool_output_replacement("call_1", &raw_output, stable_preview)
            .unwrap()
            .unwrap();
        store
            .record_tool_result_completed(
                "turn_1",
                "call_1",
                true,
                "fallback preview",
                Some(&result_ref),
                None,
                raw_output.chars().count(),
            )
            .unwrap();
        store.complete_turn("turn_1", "done", None).unwrap();
    }

    let resumed = StateStore::new(&paths).unwrap();
    let history = resumed.project_history(None).unwrap();
    let tool_message = history
        .messages
        .iter()
        .find(|message| message.role == "tool")
        .unwrap();

    assert_eq!(history.stats.tail_turns, 1);
    assert_eq!(tool_message.tool_call_id.as_deref(), Some("call_1"));
    assert!(matches!(
        tool_message.content.as_ref(),
        Some(ChatContent::Text(text)) if text == stable_preview
    ));
    assert!(!history
        .messages
        .iter()
        .any(|message| matches!(message.content.as_ref(), Some(ChatContent::Text(text)) if text.contains("fallback preview") || text.contains(&raw_output))));
    assert_eq!(resumed.tool_history_summary().unwrap().replacement_count, 1);
}

#[test]
fn tool_result_ref_reader_accepts_session_relative_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let result_ref = store
        .save_clipped_tool_output_replacement("call_1", "完整输出", "预览")
        .unwrap()
        .unwrap();

    assert_eq!(store.read_tool_result_ref(&result_ref).unwrap(), "完整输出");
}

#[test]
fn tool_result_ref_reader_rejects_paths_outside_session() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();

    assert!(store.read_tool_result_ref("../config.json").is_err());
    assert!(store
        .read_tool_result_ref(temp.path().to_string_lossy().as_ref())
        .is_err());
}

#[test]
fn session_snapshot_rebuilds_resume_visible_state_after_store_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    let raw_output = "large command output ".repeat(1_000);
    {
        let store = StateStore::new(&paths).unwrap();
        store.start_turn("turn_1", "inspect logs").unwrap();
        store
            .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
            .unwrap();
        let result_ref = store
            .save_clipped_tool_output_replacement("call_1", &raw_output, "stable log preview")
            .unwrap()
            .unwrap();
        store
            .record_tool_result_completed(
                "turn_1",
                "call_1",
                true,
                "fallback log preview",
                Some(&result_ref),
                None,
                raw_output.chars().count(),
            )
            .unwrap();
        store.complete_turn("turn_1", "done", None).unwrap();
        crate::state::session_memory::repository::upsert_memory(
            &store.conv_db,
            crate::state::session_memory::model::NewSessionMemory {
                session_id: store.session_id().to_string(),
                summary: "operator is resuming log inspection".to_string(),
                last_summarized_turn_id: Some("turn_1".to_string()),
                last_summarized_seq: 1,
                checkpoint_id: None,
                source_turn_count: 1,
                token_estimate: 48,
            },
        )
        .unwrap();
    }

    let resumed = StateStore::new(&paths).unwrap();
    let snapshot = resumed.session_snapshot(10_000).unwrap();
    let history = resumed.project_history(None).unwrap();
    let tool_message = history
        .messages
        .iter()
        .find(|message| message.role == "tool")
        .unwrap();

    assert_eq!(snapshot.session_id, "default");
    assert_eq!(snapshot.turn_count, 1);
    assert_eq!(snapshot.tool_history.call_count, 1);
    assert_eq!(snapshot.tool_history.result_count, 1);
    assert_eq!(snapshot.tool_history.replacement_count, 1);
    let memory = snapshot
        .session_memory
        .expect("session memory survives resume");
    assert_eq!(memory.last_summarized_turn_id.as_deref(), Some("turn_1"));
    assert_eq!(memory.last_summarized_seq, 1);
    assert_eq!(memory.token_estimate, 48);
    assert!(matches!(
        tool_message.content.as_ref(),
        Some(ChatContent::Text(text)) if text == "stable log preview"
    ));
    assert!(!history
        .messages
        .iter()
        .any(|message| matches!(message.content.as_ref(), Some(ChatContent::Text(text)) if text.contains("fallback log preview") || text.contains(&raw_output))));
}

#[test]
fn compaction_prompt_records_missing_tool_replacement() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "inspect file").unwrap();
    store
        .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
        .unwrap();
    store
        .record_tool_result_completed(
            "turn_1",
            "call_1",
            true,
            "preview",
            Some("tool-results/call_1.txt"),
            None,
            10_000,
        )
        .unwrap();
    store.complete_turn("turn_1", "done", None).unwrap();
    for index in 2..=3 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "tail").unwrap();
        store.complete_turn(&turn_id, "tail", None).unwrap();
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplatesConfig::default().compaction;

    let prompt = store
        .build_compaction_summary_prompt(&request, 10_000, &template)
        .unwrap();
    let recovery = store.recovery_snapshot().unwrap();

    assert!(prompt.user.contains("tool-results/call_1.txt"));
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryReplacementMissing
    );
}

#[test]
fn compaction_prompt_records_missing_tool_result_ref_file() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "inspect file").unwrap();
    store
        .record_tool_call_started("turn_1", 1, "call_1", "read_file", "{}")
        .unwrap();
    store
        .record_tool_result_completed(
            "turn_1",
            "call_1",
            true,
            "preview",
            Some("tool-results/missing.txt"),
            None,
            10_000,
        )
        .unwrap();
    store.complete_turn("turn_1", "done", None).unwrap();
    for index in 2..=3 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "tail").unwrap();
        store.complete_turn(&turn_id, "tail", None).unwrap();
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplatesConfig::default().compaction;

    let prompt = store
        .build_compaction_summary_prompt(&request, 10_000, &template)
        .unwrap();
    let recovery = store.recovery_snapshot().unwrap();

    assert!(prompt.user.contains("tool-results/missing.txt"));
    assert!(recovery
        .latest
        .as_ref()
        .unwrap()
        .reason
        .contains("完整输出引用文件缺失"));
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryReplacementMissing
    );
}

#[test]
fn compaction_prompt_rejects_over_budget_history() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", &"user ".repeat(1_000)).unwrap();
    store
        .complete_turn("turn_1", &"assistant ".repeat(1_000), None)
        .unwrap();
    for index in 2..=3 {
        let turn_id = format!("turn_{index}");
        store.start_turn(&turn_id, "tail").unwrap();
        store.complete_turn(&turn_id, "tail", None).unwrap();
    }
    let request = store.select_manual_compaction(0).unwrap().unwrap();
    let template = crate::config::PromptTemplatesConfig::default().compaction;

    let err = store
        .build_compaction_summary_prompt(&request, 500, &template)
        .unwrap_err();

    assert!(format!("{err:#}").contains("tool history summary prompt over budget"));
}

#[test]
fn provider_projection_blocks_missing_tool_result() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let projection = project_provider_turn_from_messages(
        &[
            assistant_tool_call("call_1"),
            ChatMessage::plain("user", "next"),
        ],
        0,
        10_000,
    );

    let err = store
        .enforce_provider_projection(Some("turn_1"), &projection)
        .unwrap_err();
    let recovery = store.recovery_snapshot().unwrap();

    assert!(format!("{err:#}").contains("tool call without result"));
    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryMissingResult
    );
}

#[test]
fn provider_projection_blocks_orphan_tool_result() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let projection = project_provider_turn_from_messages(
        &[
            ChatMessage::tool("call_orphan", "orphan"),
            ChatMessage::plain("user", "next"),
        ],
        0,
        10_000,
    );

    store
        .enforce_provider_projection(Some("turn_1"), &projection)
        .unwrap_err();
    let recovery = store.recovery_snapshot().unwrap();

    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryOrphanResult
    );
}

#[test]
fn provider_projection_blocks_duplicate_tool_result() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let projection = project_provider_turn_from_messages(
        &[
            assistant_tool_call("call_1"),
            ChatMessage::tool("call_1", "first"),
            ChatMessage::tool("call_1", "second"),
            ChatMessage::plain("user", "next"),
        ],
        0,
        10_000,
    );

    store
        .enforce_provider_projection(Some("turn_1"), &projection)
        .unwrap_err();
    let recovery = store.recovery_snapshot().unwrap();

    assert_eq!(
        recovery.latest.as_ref().unwrap().kind,
        FailureKind::ToolHistoryDuplicateResult
    );
}

fn assistant_tool_call(call_id: &str) -> ChatMessage {
    ChatMessage::assistant(
        "",
        Some(vec![ToolCall {
            id: call_id.to_string(),
            kind: "function".to_string(),
            function: ToolCallFunction {
                name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
    )
}

/// 在运行中轮次写入若干条工具调用与结果。
///
/// 参数:
/// - `store`: 状态仓库
/// - `turn_id`: 运行中轮次标识
/// - `rounds`: 模型子轮数量
/// - `output`: 每条工具结果正文
///
/// 返回:
/// - 无
fn record_running_turn_tool_rounds(store: &StateStore, turn_id: &str, rounds: usize, output: &str) {
    for round in 1..=rounds {
        let call_id = format!("call_{round}");
        store
            .record_tool_call_started_with_context(
                turn_id,
                round,
                round,
                None,
                &call_id,
                "read_file",
                r#"{"path":"src/main.rs"}"#,
            )
            .unwrap();
        store
            .record_tool_result_completed(
                turn_id,
                &call_id,
                true,
                output,
                None,
                None,
                output.chars().count(),
            )
            .unwrap();
    }
}

/// 验证单轮内大量工具调用可以被压缩掉，而不是原样回放。
///
/// 这是旧策略完全失效的场景：用户提一个问题，模型在给出最终答复前连续调用
/// 数十次工具，全部挂在同一个 Running 轮次上。旧策略 filter 掉 Running 后
/// 无可压缩内容，压缩形同虚设。
#[test]
fn running_turn_tool_calls_are_compacted_out_of_context() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "重构这个模块").unwrap();
    let bulky = "tool output line ".repeat(200);
    record_running_turn_tool_rounds(&store, "turn_1", 30, &bulky);

    let before = store.project_running_turn_tool_messages("turn_1").unwrap();
    let request = store
        .select_manual_compaction(0)
        .unwrap()
        .expect("running turn must be compactable");
    let running = request
        .running_turn
        .as_ref()
        .expect("running turn range must be selected");
    assert_eq!(running.turn_id, "turn_1");
    assert_eq!(
        running.compacted_calls,
        30 - crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS
    );

    store
        .apply_compaction(&request, "我已读完主要文件，下一步改 c.rs")
        .unwrap();
    let after = store.project_running_turn_tool_messages("turn_1").unwrap();

    // 1. 压缩后运行中轮次的工具消息大幅减少
    assert!(
        after.len() < before.len(),
        "压缩必须真正移除工具消息: before={} after={}",
        before.len(),
        after.len()
    );
    // 2. 只剩保留的末尾若干子轮（每子轮 1 条 assistant + 1 条 tool）
    assert_eq!(
        after.len(),
        crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS * 2
    );
    // 3. 剩余消息的 tool_call 与结果严格配对，不能出现孤立 tool_call
    assert_tool_calls_are_paired(&after);
}

/// 验证压缩边界在多次压缩之间累计推进。
#[test]
fn repeated_running_turn_compaction_advances_the_boundary() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    store.start_turn("turn_1", "长任务").unwrap();
    let bulky = "payload ".repeat(200);
    record_running_turn_tool_rounds(&store, "turn_1", 20, &bulky);

    let first = store.select_manual_compaction(0).unwrap().unwrap();
    let first_boundary = first.running_turn.as_ref().unwrap().compacted_calls;
    store.apply_compaction(&first, "第一次笔记").unwrap();

    // 新增更多工具调用后再次压缩
    for round in 21..=32 {
        let call_id = format!("call_{round}");
        store
            .record_tool_call_started_with_context(
                "turn_1",
                round,
                round,
                None,
                &call_id,
                "read_file",
                "{}",
            )
            .unwrap();
        store
            .record_tool_result_completed(
                "turn_1",
                &call_id,
                true,
                &bulky,
                None,
                None,
                bulky.chars().count(),
            )
            .unwrap();
    }

    let second = store.select_manual_compaction(0).unwrap().unwrap();
    let second_boundary = second.running_turn.as_ref().unwrap().compacted_calls;

    assert!(
        second_boundary > first_boundary,
        "第二次压缩必须接着上一次的位置往后推进: first={first_boundary} second={second_boundary}"
    );
    store.apply_compaction(&second, "第二次笔记").unwrap();
    let after = store.project_running_turn_tool_messages("turn_1").unwrap();
    assert_eq!(
        after.len(),
        crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS * 2
    );
    assert_tool_calls_are_paired(&after);
}

/// 断言消息序列中每个 tool_call 都有紧随其后的配对结果。
///
/// provider 会拒绝孤立的 tool_call，切分点必须落在子轮边界上。
///
/// 参数:
/// - `messages`: 待校验消息序列
///
/// 返回:
/// - 无；不配对时断言失败
fn assert_tool_calls_are_paired(messages: &[ChatMessage]) {
    let mut pending = 0usize;
    for message in messages {
        match message.role.as_str() {
            "assistant" => {
                let calls = message
                    .tool_calls
                    .as_ref()
                    .map(Vec::len)
                    .unwrap_or_default();
                assert_eq!(pending, 0, "上一子轮的工具结果不完整");
                pending = calls;
            }
            "tool" => {
                assert!(pending > 0, "出现没有对应 tool_call 的孤立工具结果");
                pending -= 1;
            }
            _ => {}
        }
    }
    assert_eq!(pending, 0, "存在没有结果的 tool_call");
}

/// 验证压缩后的历史投影只保留用户消息。
///
/// 方案 B 的核心断言：assistant 与 tool 消息全部由交接笔记覆盖，
/// 不再回放；因此也不存在孤立 tool_call 的风险。
#[test]
fn compacted_history_projection_keeps_only_user_messages() {
    let temp = tempfile::tempdir().unwrap();
    let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
    let bulky = "tool payload ".repeat(200);
    for index in 1..=3 {
        let turn_id = format!("turn_{index}");
        store
            .start_turn(&turn_id, &format!("第 {index} 个问题"))
            .unwrap();
        let call_id = format!("call_{index}");
        store
            .record_tool_call_started(&turn_id, 1, &call_id, "read_file", "{}")
            .unwrap();
        store
            .record_tool_result_completed(
                &turn_id,
                &call_id,
                true,
                &bulky,
                None,
                None,
                bulky.chars().count(),
            )
            .unwrap();
        store
            .complete_turn(&turn_id, &format!("第 {index} 个回答"), None)
            .unwrap();
    }
    // 新开一轮承载压缩后的用户消息
    store.start_turn("turn_4", "第 4 个问题").unwrap();
    store.complete_turn("turn_4", "第 4 个回答", None).unwrap();

    let request = store.select_manual_compaction(0).unwrap().unwrap();
    store.apply_compaction(&request, "交接笔记正文").unwrap();
    let history = store.project_history(None).unwrap();

    assert!(
        history
            .messages
            .iter()
            .all(|message| message.role == "user"),
        "压缩后不得残留 assistant 或 tool 消息: {:?}",
        history
            .messages
            .iter()
            .map(|message| message.role.clone())
            .collect::<Vec<_>>()
    );
    assert!(history
        .checkpoint_context
        .as_ref()
        .unwrap()
        .contains("交接笔记正文"));
}

/// 验证交接笔记消息在下一次压缩时不会被当作用户输入保留。
#[test]
fn handoff_note_does_not_survive_as_user_input() {
    let note = crate::state::compaction::summary_context_message("我正在重构 X 模块");
    let message = ChatMessage::plain("user", note);

    assert!(
        !crate::state::compaction::is_real_user_input(&message),
        "交接笔记必须在下一次压缩时被排除，否则摘要会逐次嵌套"
    );
}

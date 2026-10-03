use crate::llm::ChatMessage;
use crate::paths::SaiPaths;
use crate::state::compaction::estimate_chat_messages_tokens;
use crate::state::context_blocks::CompressRequest;
use crate::state::request_projection::project_provider_turn_from_messages;
use crate::state::tool_history::ToolResultOutput;
use crate::state::{CompactionApplyOutcome, StateStore};
use std::collections::HashSet;

/// 【上下文】【组合核验】写入指定数量的真实工具记录
/// 参数: state 为状态，turn 为轮次，count 为条数，completed 表示是否结束轮次；返回无
fn seed(state: &StateStore, turn: &str, count: usize, completed: bool) {
    state.start_turn(turn, "inspect source").unwrap();
    let output = "source entry path=/src/example.rs status=ok\n".repeat(300);
    for index in 0..count {
        let id = format!("{turn}-{index}");
        state
            .record_tool_call_started_with_context(
                turn,
                index + 1,
                crate::state::tool_history::ToolAssistantContext {
                    assistant_round: index + 1,
                    assistant_reasoning: None,
                },
                &id,
                "read_file",
                "{}",
            )
            .unwrap();
        state
            .record_tool_result_completed(
                turn,
                &id,
                true,
                ToolResultOutput {
                    result_preview: &output,
                    result_ref: None,
                    error: None,
                    original_chars: output.chars().count(),
                },
            )
            .unwrap();
    }
    if completed {
        state.complete_turn(turn, "done", None).unwrap();
    }
}

/// 【上下文】【组合核验】以正式投影器重建当前运行轮次
/// 参数: state 为状态，turn 为轮次；返回系统、用户与工具消息
fn running_request(state: &StateStore, turn: &str) -> Vec<ChatMessage> {
    let history = state.project_history(Some(turn)).unwrap();
    let mut messages = vec![ChatMessage::system("test system")];
    if let Some(summary) = history.checkpoint_context {
        messages.push(ChatMessage::system(summary));
    }
    messages.extend(history.messages);
    messages.push(ChatMessage::plain("user", "inspect source"));
    messages.extend(state.project_running_turn_tool_messages(turn).unwrap());
    state.apply_context_blocks(&mut messages).unwrap();
    messages
}

/// 【上下文】【组合核验】局部压缩后，能装入窗口的轮次内全局摘要必须通过预算预检
/// 参数: 无；返回无，错误拒绝时测试失败
#[test]
fn running_turn_global_fallback_accepts_a_fitting_projection() {
    let root = tempfile::tempdir().unwrap();
    let state = StateStore::new(&SaiPaths::for_tests(root.path())).unwrap();
    seed(&state, "running", 16, false);
    state
        .compress_context_block(&CompressRequest {
            message_ids: vec!["running-0".into()],
            topic: "检查结果".into(),
            summary: "已检查 /src/example.rs，状态正常。".into(),
            expected_revision: 0,
        })
        .unwrap();
    let before = running_request(&state, "running");
    let before_tokens = estimate_chat_messages_tokens(&before);
    let limit = before_tokens / 2;
    let mut projection = project_provider_turn_from_messages(&before, 0, limit);
    projection.context_blocks = true;
    let request = state
        .select_compaction_for_projection(&projection, true)
        .unwrap()
        .unwrap();
    assert!(request.compact_turns.is_empty());
    assert_eq!(request.running_turn.as_ref().unwrap().compacted_calls, 12);
    let summary = "已经检查前十二项源码，状态正常，继续处理末尾四项。";
    let budget = state
        .compaction_budget_check(&request, summary, &projection, Some("running"))
        .unwrap();
    let outcome = state
        .apply_compaction_with_budget_guard(&request, summary, &projection, Some("running"))
        .unwrap();
    if outcome != CompactionApplyOutcome::Applied {
        // 1. 【上下文】【组合核验】独立计算预检拒绝后的实际投影，验证拒绝是否正确
        state.apply_compaction(&request, summary).unwrap();
    }
    let rebuilt = running_request(&state, "running");
    assert_eq!(
        rebuilt
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        4,
        "测试前提：四条近期工具结果必须仍在正式投影中"
    );
    let actual = estimate_chat_messages_tokens(&rebuilt);
    assert!(
        budget.result_chars.abs_diff(actual) < 32,
        "预算应与重建请求一致，仅允许 checkpoint 标识差异"
    );
    assert!(actual < limit, "测试前提：压缩后的实际请求可以装入窗口");
    assert_eq!(
        outcome,
        CompactionApplyOutcome::Applied,
        "实际请求可以装入窗口，预算预检不应拒绝"
    );
}

/// 【上下文】【组合核验】全局摘要后切换分支，候选必须属于实际发送的历史
/// 参数: 无；返回无，存在不可投影的候选时失败
#[test]
fn compression_candidates_exist_in_the_provider_projection_after_branch_switch() {
    let root = tempfile::tempdir().unwrap();
    let state = StateStore::new(&SaiPaths::for_tests(root.path())).unwrap();
    seed(&state, "branch-a", 8, true);
    state.switch_to_session_start().unwrap();
    seed(&state, "branch-b", 8, true);
    let compact = state.select_manual_compaction(0).unwrap().unwrap();
    state
        .apply_compaction(&compact, "branch-b completed")
        .unwrap();
    state.switch_active_leaf("branch-a").unwrap();
    let messages = state.project_history(None).unwrap().messages;
    let projected = messages
        .iter()
        .filter_map(|message| message.tool_call_id.as_deref())
        .collect::<HashSet<_>>();
    let catalog = state.context_block_catalog(0, 50, 0).unwrap();
    let missing = catalog
        .candidates
        .iter()
        .filter(|candidate| {
            candidate.eligible && !projected.contains(candidate.message_id.as_str())
        })
        .map(|candidate| candidate.message_id.clone())
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "工具不应报告实际请求中不存在的可压缩结果"
    );
}

/// 【上下文】【连续压缩回归】重复压缩及重启后，候选只包含仍在请求中的工具结果
/// 参数: 无；返回无
#[test]
fn successive_running_compactions_share_boundaries_with_catalog_and_resume() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = StateStore::new(&paths).unwrap();
    seed(&state, "running", 20, false);
    for boundary in [8, 16] {
        let before = running_request(&state, "running");
        let mut projection = project_provider_turn_from_messages(&before, 0, 100_000);
        projection.context_blocks = true;
        let request = crate::state::compaction::CompactionRequest::new(Vec::new(), None)
            .with_running_turn(Some(crate::state::compaction::RunningTurnCompaction {
                turn_id: "running".into(),
                compacted_calls: boundary,
            }));
        let budget = state
            .compaction_budget_check(&request, "已完成前面源码检查", &projection, Some("running"))
            .unwrap();
        state
            .apply_compaction(&request, "已完成前面源码检查")
            .unwrap();
        let after = running_request(&state, "running");
        assert!(
            budget
                .result_chars
                .abs_diff(estimate_chat_messages_tokens(&after))
                < 32
        );
        let catalog = state.context_block_catalog(0, 50, 0).unwrap();
        assert_eq!(catalog.total, 20 - boundary);
        let ids = after
            .iter()
            .filter_map(|message| message.tool_call_id.as_deref())
            .collect::<HashSet<_>>();
        assert!(catalog
            .candidates
            .iter()
            .all(|candidate| ids.contains(candidate.message_id.as_str())));
    }
    state.complete_turn("running", "done", None).unwrap();
    drop(state);
    let resumed = StateStore::new(&paths).unwrap();
    let history = resumed.project_history(None).unwrap();
    assert_eq!(
        history
            .messages
            .iter()
            .filter(|message| message.role == "tool")
            .count(),
        4
    );
    assert_eq!(resumed.context_block_catalog(0, 50, 0).unwrap().total, 4);
}

/// 【上下文】【预算拒绝回归】近期工具、图片、间隙消息与临时提示仍超限时不得提交全局摘要
/// 参数: 无；返回无
#[test]
fn budget_preserves_recent_attachments_and_rejects_true_overflow() {
    use crate::state::turn_messages::{NewTurnMessage, TurnMessageKind};
    let root = tempfile::tempdir().unwrap();
    let state = StateStore::new(&SaiPaths::for_tests(root.path())).unwrap();
    seed(&state, "running", 16, false);
    state
        .record_turn_message(NewTurnMessage {
            turn_id: "running".into(),
            after_tool_seq: 15,
            kind: TurnMessageKind::QueuedUser,
            model_content: "保留这条新要求".into(),
            display_content: "保留这条新要求".into(),
            reasoning: None,
            image_urls: vec!["data:image/png;base64,aW1hZ2U=".into()],
        })
        .unwrap();
    let extra = ChatMessage::plain("user", "临时指令 ".repeat(1000));
    let mut before = running_request(&state, "running");
    before.push(extra.clone());
    let projection = project_provider_turn_from_messages(&before, 0, 1000);
    let request = state
        .select_compaction_for_projection(&projection, true)
        .unwrap()
        .unwrap();
    let budget = state
        .compaction_budget_check(&request, "已完成前十二次检查", &projection, Some("running"))
        .unwrap();
    assert_eq!(
        state
            .apply_compaction_with_budget_guard(
                &request,
                "已完成前十二次检查",
                &projection,
                Some("running")
            )
            .unwrap(),
        CompactionApplyOutcome::RejectedOverBudget
    );
    assert!(state.running_turn_compaction_boundary().unwrap().is_none());
    state
        .apply_compaction(&request, "已完成前十二次检查")
        .unwrap();
    let mut after = running_request(&state, "running");
    after.push(extra);
    let serialized = serde_json::to_string(&after).unwrap();
    assert!(serialized.contains("保留这条新要求"));
    assert!(serialized.contains("data:image/png;base64,aW1hZ2U="));
    assert!(
        budget
            .result_chars
            .abs_diff(estimate_chat_messages_tokens(&after))
            < 32
    );
}

/// 【上下文】【模型用量回归】供应商额外报告的占用不能在重建历史时被当作零处理
/// 参数: 无；返回无
#[test]
fn provider_usage_overhead_survives_compaction_budget_reprojection() {
    let root = tempfile::tempdir().unwrap();
    let state = StateStore::new(&SaiPaths::for_tests(root.path())).unwrap();
    seed(&state, "running", 16, false);
    let before = running_request(&state, "running");
    let mut projection = project_provider_turn_from_messages(&before, 0, 100_000);
    let provider_overhead = 5000;
    projection.estimate.message_chars += provider_overhead;
    let request = state
        .select_compaction_for_projection(&projection, true)
        .unwrap()
        .unwrap();
    let budget = state
        .compaction_budget_check(&request, "旧子轮已完成", &projection, Some("running"))
        .unwrap();
    state.apply_compaction(&request, "旧子轮已完成").unwrap();
    let after = running_request(&state, "running");
    assert!(
        budget
            .result_chars
            .abs_diff(estimate_chat_messages_tokens(&after) + provider_overhead)
            < 32
    );
}

use super::CompressRequest;
use crate::llm::{ChatContent, ChatMessage};
use crate::paths::SaiPaths;
use crate::state::{tool_history::ToolResultOutput, StateStore};

/// 【上下文】【测试数据】创建隔离会话与一轮八条工具结果
/// 参数: 无；返回临时目录、路径、状态和原文
fn fixture() -> (tempfile::TempDir, SaiPaths, StateStore, String) {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let state = StateStore::new(&paths).unwrap();
    let original = "配置 path=/src/main.rs, result=ok\n".repeat(300);
    seed_turn(&state, "first", &original);
    (root, paths, state, original)
}

/// 【上下文】【测试数据】写入并完成带工具结果的轮次
/// 参数: state 为状态，turn 为轮次，original 为输出；返回无
fn seed_turn(state: &StateStore, turn: &str, original: &str) {
    state.start_turn(turn, "inspect source").unwrap();
    for index in 0..8 {
        let id = format!("{turn}-{index}");
        state
            .record_tool_call_started(
                turn,
                index + 1,
                &id,
                "read_file",
                r#"{"path":"src/main.rs"}"#,
            )
            .unwrap();
        state
            .record_tool_result_completed(
                turn,
                &id,
                true,
                ToolResultOutput {
                    result_preview: original,
                    result_ref: None,
                    error: None,
                    original_chars: original.chars().count(),
                },
            )
            .unwrap();
    }
    state
        .complete_turn(turn, "inspection complete", None)
        .unwrap();
}

/// 【上下文】【测试请求】创建简短的单块提交
/// 参数: ids 为结果引用，revision 为版本；返回请求
fn request(ids: &[&str], revision: u64) -> CompressRequest {
    CompressRequest {
        message_ids: ids.iter().map(|id| id.to_string()).collect(),
        summary: "检查 src/main.rs，配置有效，结果为 ok。".into(),
        topic: "配置检查".into(),
        expected_revision: revision,
    }
}

/// 【上下文】【投影回归】验证摘要节省、配对不变、原始历史不变及重复提交
/// 参数: 无；返回无，断言失败时终止
#[test]
fn compression_preserves_pairs_and_original_history() {
    let (_root, _paths, state, original) = fixture();
    let req = request(&["first-0", "first-1"], 0);
    let block = state.compress_context_block(&req).unwrap();
    assert!(block.before_tokens > block.after_tokens + 128);
    let mut messages = state.project_history(None).unwrap().messages;
    let before = messages.clone();
    assert!(state.apply_context_blocks(&mut messages).unwrap());
    assert_eq!(messages.len(), before.len());
    for (old, new) in before.iter().zip(&messages) {
        assert_eq!(old.role, new.role);
        assert_eq!(old.tool_call_id, new.tool_call_id);
        assert_eq!(
            serde_json::to_value(&old.tool_calls).unwrap(),
            serde_json::to_value(&new.tool_calls).unwrap()
        );
        if old.role != "tool" {
            assert_eq!(
                serde_json::to_value(old).unwrap(),
                serde_json::to_value(new).unwrap()
            );
        }
    }
    assert!(!state.apply_context_blocks(&mut messages).unwrap());
    assert_eq!(
        state.compress_context_block(&req).unwrap().block_id,
        block.block_id
    );
    assert_eq!(state.context_block_catalog(0, 50, 0).unwrap().revision, 1);
    assert!(state
        .project_history(None)
        .unwrap()
        .messages
        .iter()
        .any(|message| {
            message.tool_call_id.as_deref() == Some("first-0")
                && matches!(&message.content, Some(ChatContent::Text(text)) if text == &original)
        }));
}

/// 【上下文】【完整回读】验证归档引用删除与重启后，Unicode 和空字符仍逐字恢复
/// 参数: 无；返回无
#[test]
fn archived_output_survives_file_removal_and_restart_with_unicode_paging() {
    let (_root, paths, state, preview) = fixture();
    let full = format!("start\0{}\n终点", "中文e\u{301}原文".repeat(500));
    let reference = state
        .save_clipped_tool_output_replacement("first-0", &full, &preview)
        .unwrap()
        .unwrap();
    state
        .compress_context_block(&request(&["first-0"], 0))
        .unwrap();
    std::fs::remove_file(state.state_dir().join(reference)).unwrap();
    drop(state);
    let state = StateStore::new(&paths).unwrap();
    let mut restored = String::new();
    let mut offset = 0;
    loop {
        let page = state
            .restore_context_message("first-0", offset, 113)
            .unwrap();
        assert_eq!(page.total_chars, full.chars().count());
        restored.push_str(&page.content);
        match page.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(restored, full);
    assert!(state
        .restore_context_message("first-0", full.chars().count() + 1, 10)
        .is_err());
}

/// 【上下文】【事务拒绝】验证重复引用、旧版本、重叠及无收益提交均不留下部分结果
/// 参数: 无；返回无
#[test]
fn invalid_submissions_are_atomic() {
    let (_root, _paths, state, _original) = fixture();
    for req in [
        request(&["first-0", "first-0"], 0),
        request(&["first-0"], 9),
        request(&["first-0", "unknown"], 0),
    ] {
        assert!(state.compress_context_block(&req).is_err());
    }
    let mut oversized = request(&["first-0"], 0);
    oversized.summary = "字".repeat(6000);
    assert!(state.compress_context_block(&oversized).is_err());
    assert_eq!(state.context_block_catalog(0, 20, 0).unwrap().revision, 0);
    state
        .compress_context_block(&request(&["first-0"], 0))
        .unwrap();
    assert!(state
        .compress_context_block(&request(&["first-0", "first-1"], 1))
        .is_err());
    assert!(state.restore_context_message("first-1", 0, 100).is_err());
    assert_eq!(state.context_block_catalog(0, 20, 0).unwrap().revision, 1);
}

/// 【上下文】【候选保护】验证近期结果、错误和上下文管理工具不能被压缩
/// 参数: 无；返回无
#[test]
fn protects_recent_errors_and_management_results() {
    let (_root, _paths, state, original) = fixture();
    state
        .record_tool_result_completed(
            "first",
            "first-0",
            false,
            ToolResultOutput {
                result_preview: &original,
                result_ref: None,
                error: Some("failed"),
                original_chars: original.chars().count(),
            },
        )
        .unwrap();
    state
        .record_tool_call_display("first-1", "restore_context", "{}")
        .unwrap();
    let status = state.context_block_catalog(0, 50, 0).unwrap();
    assert_eq!(
        status.candidates[0].protected_reason.as_deref(),
        Some("error_or_unfinished")
    );
    assert_eq!(
        status.candidates[1].protected_reason.as_deref(),
        Some("context_management")
    );
    assert!(status.candidates[2].eligible);
    assert!(status.candidates[4..]
        .iter()
        .all(|item| item.protected_reason.as_deref() == Some("recent_working_set")));
    for id in ["first-0", "first-1", "first-7"] {
        assert!(state.compress_context_block(&request(&[id], 0)).is_err());
    }
}

/// 【上下文】【归档失败】缺失引用、超限文件和不完整预览必须拒绝整次提交
/// 参数: 无；返回无
#[test]
fn incomplete_or_oversized_originals_never_commit() {
    let (_root, _paths, state, preview) = fixture();
    let reference = state
        .save_clipped_tool_output_replacement("first-1", &format!("{preview}extra"), &preview)
        .unwrap()
        .unwrap();
    let path = state.state_dir().join(&reference);
    std::fs::remove_file(&path).unwrap();
    assert!(state
        .compress_context_block(&request(&["first-0", "first-1"], 0))
        .is_err());
    std::fs::File::create(path)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    assert!(state
        .compress_context_block(&request(&["first-1"], 0))
        .is_err());
    state
        .record_tool_result_completed(
            "first",
            "first-2",
            true,
            ToolResultOutput {
                result_preview: &preview,
                result_ref: None,
                error: None,
                original_chars: preview.chars().count() + 1,
            },
        )
        .unwrap();
    assert!(state
        .compress_context_block(&request(&["first-2"], 0))
        .is_err());
    assert_eq!(state.context_block_catalog(0, 20, 0).unwrap().revision, 0);
    assert!(state.search_context_blocks("配置", 20).unwrap().is_empty());
}

/// 【上下文】【分支隔离】归档只在其原分支或共享祖先可见
/// 参数: 无；返回无
#[test]
fn branch_switch_hides_unrelated_archives() {
    let (_root, _paths, state, original) = fixture();
    state
        .compress_context_block(&request(&["first-0"], 0))
        .unwrap();
    state.switch_to_session_start().unwrap();
    seed_turn(&state, "other", &original);
    assert!(state
        .context_block_catalog(0, 20, 0)
        .unwrap()
        .blocks
        .is_empty());
    assert!(state.search_context_blocks("配置", 20).unwrap().is_empty());
    assert!(state.restore_context_message("first-0", 0, 100).is_err());
    assert!(state
        .compress_context_block(&request(&["first-1"], 1))
        .is_err());
    state.switch_active_leaf("first").unwrap();
    assert!(!state.search_context_blocks("配置", 20).unwrap().is_empty());
}

/// 【上下文】【全局压缩】删除旧轮次并连续生成 checkpoint 后仍可回读原文
/// 参数: 无；返回无
#[test]
fn archive_survives_global_compaction_and_reset_removes_it() {
    let (_root, paths, state, original) = fixture();
    state
        .compress_context_block(&request(&["first-0"], 0))
        .unwrap();
    for cycle in 0..2 {
        seed_turn(&state, &format!("later-{cycle}"), &original);
        let compact = state.select_manual_compaction(0).unwrap().unwrap();
        state.apply_compaction(&compact, "配置检查已完成").unwrap();
        assert_eq!(
            state
                .restore_context_message("first-0", 0, 20)
                .unwrap()
                .content,
            original.chars().take(20).collect::<String>()
        );
    }
    assert!(state
        .conv_db
        .load_turns()
        .unwrap()
        .iter()
        .all(|turn| turn.turn_id != "first"));
    drop(state);
    let state = StateStore::new(&paths).unwrap();
    assert!(!state.search_context_blocks("path", 10).unwrap().is_empty());
    state.reset_conversation().unwrap();
    assert!(state.restore_context_message("first-0", 0, 10).is_err());
    let status = state.context_block_catalog(0, 20, 0).unwrap();
    assert_eq!(status.revision, 0);
    assert!(status.blocks.is_empty());
}

/// 【上下文】【投影原子性】缺少组成员或存在多模态正文时，不产生悬空摘要引用
/// 参数: 无；返回无
#[test]
fn incomplete_projection_keeps_all_members_unchanged() {
    let (_root, _paths, state, _original) = fixture();
    state
        .compress_context_block(&request(&["first-0", "first-1"], 0))
        .unwrap();
    let mut messages = vec![ChatMessage::tool("first-1", "untouched")];
    assert!(!state.apply_context_blocks(&mut messages).unwrap());
    let mut missing_text = ChatMessage::tool("first-0", "x");
    missing_text.content = None;
    messages.push(missing_text);
    assert!(!state.apply_context_blocks(&mut messages).unwrap());
}

/// 【上下文】【并发提交】两个相同版本的竞争提交只能有一个成功
/// 参数: 无；返回无
#[test]
fn competing_submissions_commit_once() {
    let (_root, _paths, state, _original) = fixture();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = ["first-0", "first-1"].map(|id| {
        let state = state.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            state.compress_context_block(&request(&[id], 0)).is_ok()
        })
    });
    assert_eq!(
        handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap().then_some(()))
            .count(),
        1
    );
    let status = state.context_block_catalog(0, 20, 0).unwrap();
    assert_eq!(status.revision, 1);
    assert_eq!(status.total_blocks, 1);
}

/// 【上下文】【并发重试】相同请求同时到达时返回同一个块且只增加一次修订号
/// 参数: 无；返回无
#[test]
fn simultaneous_identical_requests_are_idempotent() {
    let (_root, _paths, state, _original) = fixture();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = [0, 1].map(|_| {
        let state = state.clone();
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            state
                .compress_context_block(&request(&["first-0"], 0))
                .unwrap()
                .block_id
        })
    });
    let ids = handles.map(|handle| handle.join().unwrap());
    assert_eq!(ids[0], ids[1]);
    assert_eq!(state.context_block_catalog(0, 20, 0).unwrap().revision, 1);
}

/// 【上下文】【共享祖先】混合块含不可见后代时，不允许通过摘要关键词探测另一分支
/// 参数: 无；返回无
#[test]
fn summary_search_does_not_leak_hidden_branch_members() {
    let (_root, _paths, state, original) = fixture();
    seed_turn(&state, "child", &original);
    let mut req = request(&["first-0", "child-0"], 0);
    req.summary = "private-child-summary-only-marker".into();
    state.compress_context_block(&req).unwrap();
    assert_eq!(
        state
            .search_context_blocks("private-child-summary", 20)
            .unwrap()
            .len(),
        2
    );
    state.switch_active_leaf("first").unwrap();
    assert!(state
        .search_context_blocks("private-child-summary", 20)
        .unwrap()
        .is_empty());
    assert!(state.restore_context_message("first-0", 0, 10).is_ok());
    assert!(state.restore_context_message("child-0", 0, 10).is_err());
    assert!(state
        .context_block_catalog(0, 20, 0)
        .unwrap()
        .blocks
        .is_empty());
}

/// 【上下文】【分页边界】候选、摘要及搜索均限制输出大小且保留继续读取的偏移
/// 参数: 无；返回无
#[test]
fn catalogs_and_search_are_bounded_and_paginated() {
    let (_root, _paths, state, original) = fixture();
    seed_turn(&state, "next", &original);
    for index in 0..6 {
        state
            .compress_context_block(&request(&[&format!("first-{index}")], index))
            .unwrap();
    }
    let status = state.context_block_catalog(0, 3, 0).unwrap();
    assert_eq!(status.candidates.len(), 3);
    assert_eq!(status.next_offset, Some(3));
    assert_eq!(status.blocks.len(), 5);
    assert_eq!(status.total_blocks, 6);
    assert_eq!(status.next_block_offset, Some(5));
    let end = state.context_block_catalog(999, 3, 5).unwrap();
    assert!(end.candidates.is_empty());
    assert_eq!(end.blocks.len(), 1);
    assert_eq!(end.next_block_offset, None);
    assert_eq!(state.search_context_blocks("path", 2).unwrap().len(), 2);
}

/// 【上下文】【预算回归】局部摘要与全局压缩组合时，预算必须等于实际重建请求
/// 参数: 无；返回无
#[test]
fn global_compaction_budget_uses_projected_block_sizes() {
    use crate::state::compaction::{estimate_chat_messages_tokens, CompactionRequest};
    use crate::state::request_projection::project_provider_turn_from_messages;
    let (_root, _paths, state, original) = fixture();
    seed_turn(&state, "second", &original);
    seed_turn(&state, "third", &original);
    state
        .compress_context_block(&request(&["first-0"], 0))
        .unwrap();
    state
        .compress_context_block(&request(&["second-0"], 1))
        .unwrap();
    let system = ChatMessage::system("preserve system instructions ".repeat(2000));
    let mut messages = vec![system.clone()];
    messages.extend(state.project_history(None).unwrap().messages);
    state.apply_context_blocks(&mut messages).unwrap();
    let mut projection = project_provider_turn_from_messages(&messages, 0, 100_000);
    projection.context_blocks = true;
    let first = state.conv_db.active_branch_turns().unwrap().remove(0);
    let compact = CompactionRequest::new(vec![first], None);
    let budget = state
        .compaction_budget_check(&compact, "first turn summarized", &projection, None)
        .unwrap();
    state
        .apply_compaction(&compact, "first turn summarized")
        .unwrap();
    let history = state.project_history(None).unwrap();
    let mut next = vec![
        system,
        ChatMessage::system(history.checkpoint_context.unwrap()),
    ];
    next.extend(history.messages);
    state.apply_context_blocks(&mut next).unwrap();
    // 1. 【上下文】【预算回归】实际 checkpoint 标识包含时间戳，允许这部分元数据的少量差异
    assert!(
        budget
            .result_chars
            .abs_diff(estimate_chat_messages_tokens(&next))
            < 32
    );
}

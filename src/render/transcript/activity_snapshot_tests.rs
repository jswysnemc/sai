use super::super::test_support::{chunk, options};
use super::*;
use crate::llm::ToolCallStreamProgress;

/// 【终端】【详情测试】新阶段替换旧详情，完成后不残留执行输出。
/// 参数: 无；返回: 无。
#[test]
fn activity_snapshot_replaces_previous_stage() {
    let mut store = TranscriptStore::new(200);
    let options = options();
    assert_eq!(store.activity_snapshot(&options).title, t("Ready", "就绪"));
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "reasoning detail"));
    assert_eq!(
        store.activity_snapshot(&options).details,
        "reasoning detail"
    );
    store.push_tool_call("read_file".into(), r#"{"path":"fixture.rs"}"#.into());
    let snapshot = store.activity_snapshot(&options);
    assert_eq!(
        snapshot.title,
        format!("{} · read_file", t("Running", "执行中"))
    );
    assert!(snapshot.details.contains("fixture.rs"));
    assert!(!snapshot.details.contains("reasoning detail"));
    store.push_tool_progress("read_file".into(), "latest progress".into());
    assert!(store
        .activity_snapshot(&options)
        .details
        .contains("latest progress"));
    store.push_tool_result("read_file".into(), true, "old output".into());
    store.push_chunk(&chunk(ChatStreamKind::Content, "ordinary answer"));
    assert_eq!(
        store.activity_snapshot(&options).title,
        t("Writing response", "生成回答")
    );
    assert!(!store
        .activity_snapshot(&options)
        .details
        .contains("old output"));
}

/// 【终端】【详情测试】隐藏偏好同时约束固定区，不泄露思考或工具参数。
/// 参数: 无；返回: 无。
#[test]
fn activity_snapshot_respects_hidden_modes() {
    let mut store = TranscriptStore::new(200);
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Hidden,
        tool_call_mode: ToolCallDisplayMode::Hidden,
    };
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "private reasoning"));
    assert!(!store
        .activity_snapshot(&options)
        .details
        .contains("private"));
    store.push_tool_call("read_file".into(), "private arguments".into());
    assert!(!store
        .activity_snapshot(&options)
        .details
        .contains("private"));
}

/// 【终端】【详情测试】实时参数更新使用同一个快照，未完成 JSON 仍可查看。
/// 参数: 无；返回: 无。
#[test]
fn activity_snapshot_shows_live_arguments() {
    let mut store = TranscriptStore::new(200);
    store.push_tool_call_progress(&ToolCallStreamProgress {
        index: 0,
        name: Some("read_file".into()),
        arguments_chars: 16,
        arguments_bytes: 16,
        arguments_preview: r#"{"path":"CURRENT"#.into(),
        edit_diff_counts: None,
    });
    let snapshot = store.activity_snapshot(&options());
    assert_eq!(
        snapshot.title,
        format!("{} · read_file", t("Preparing", "准备工具参数"))
    );
    assert!(snapshot.details.contains("CURRENT"));
}

/// 【终端】【详情测试】有界文本保留完整宽字符并清除终端控制序列。
/// 参数: 无；返回: 无。
#[test]
fn activity_snapshot_bounds_and_sanitizes_details() {
    let text = format!("{}\x1b[31m最新输出\x1b[0m\x07", "中文".repeat(3000));
    let detail = bounded_text(&text, true);
    assert!(detail.ends_with("最新输出"));
    assert!(!detail.contains('\x1b'));
    assert!(!detail.contains('\x07'));
    assert!(detail.len() <= DETAIL_BYTE_LIMIT);
    assert!(arguments_text(r#"{"path":"file.rs"}"#).contains("\n"));
    assert!(bounded_text(&"\t".repeat(9000), true).len() <= DETAIL_BYTE_LIMIT);
}

/// 【终端】【详情测试】查看子会话时不能回退到主会话的实时思考。
/// 参数: 无；返回: 无。
#[test]
fn activity_snapshot_isolates_subagent_view() {
    let mut store = TranscriptStore::new(200);
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "main private detail"));
    store.view = TranscriptView::Subagent {
        id: "missing-fixture".into(),
        label: "Child".into(),
    };
    assert!(!store
        .activity_snapshot(&options())
        .details
        .contains("private"));
    assert!(store.activity_snapshot(&options()).title.contains("Child"));
}

/// 【终端】【详情测试】长参数保留最新片段，达到面板容量后仍继续更新。
/// 参数: 无；返回: 无。
#[test]
fn activity_snapshot_keeps_latest_long_arguments() {
    let mut store = TranscriptStore::new(200);
    let preview = format!(
        "{{\"content\":\"{}LATEST-LONG-ARGUMENT",
        "old-".repeat(4000)
    );
    store.push_tool_call_progress(&ToolCallStreamProgress {
        index: 0,
        name: Some("write_file".into()),
        arguments_chars: preview.chars().count(),
        arguments_bytes: preview.len(),
        arguments_preview: preview,
        edit_diff_counts: None,
    });
    let snapshot = store.activity_snapshot(&options());
    assert!(snapshot.details.ends_with("LATEST-LONG-ARGUMENT"));
    assert!(snapshot.details.len() <= DETAIL_BYTE_LIMIT);
}

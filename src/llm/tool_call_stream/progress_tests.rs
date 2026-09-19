use super::{ToolCallProgressTracker, BODY_ARGUMENTS_PREVIEW_CHARS};
use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【行数回归】发送完整参数快照并验证实际工具行
/// 参数: tracker 为事件跟踪器，store 为显示历史，name 为工具名，arguments 为参数，expected 为行数徽标
/// 返回: 无；参数预览保持有限长度，同时工具行展示完整统计
fn assert_badge(
    tracker: &mut ToolCallProgressTracker,
    store: &mut TranscriptStore,
    name: &str,
    arguments: &str,
    expected: &str,
) {
    let progress = tracker
        .update(0, name, arguments)
        .expect("进度事件仍需发送");
    assert_eq!(progress.arguments_bytes, arguments.len());
    assert_eq!(progress.arguments_chars, arguments.chars().count());
    assert_eq!(
        progress.arguments_preview.chars().count(),
        arguments.chars().count().min(BODY_ARGUMENTS_PREVIEW_CHARS)
    );
    store.push_tool_call_progress(&progress);
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Summary,
    };
    let rendered = store
        .display_tail(80, &options)
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    assert!(
        plain.contains(expected),
        "收到 {} 字节参数，预览 {} 字符；期望 {expected}，实际 {plain}",
        progress.arguments_bytes,
        progress.arguments_preview.chars().count()
    );
}

/// 【终端】【行数回归】跨过预览上限后仍随每个新行更新计数；无参数，返回无
#[test]
fn writing_counts_continue_after_preview_limit() {
    let mut tracker = ToolCallProgressTracker::default();
    let mut store = TranscriptStore::new(100);
    let mut arguments = format!(
        r#"{{"path":"fixture.txt","content":"{}"#,
        "x".repeat(BODY_ARGUMENTS_PREVIEW_CHARS)
    );
    for count in 1..=3 {
        assert_badge(
            &mut tracker,
            &mut store,
            "write_file",
            &arguments,
            &format!("+{count} -0"),
        );
        arguments.push_str(r"\nnext");
    }
}

/// 【终端】【行数回归】长原文后面的新文本也必须参与统计；无参数，返回无
#[test]
fn replacement_counts_include_fields_after_preview_limit() {
    let mut tracker = ToolCallProgressTracker::default();
    let mut store = TranscriptStore::new(100);
    let mut arguments = format!(
        r#"{{"path":"fixture.txt","old_string":"{}\nold","new_string":"new"#,
        "x".repeat(BODY_ARGUMENTS_PREVIEW_CHARS)
    );
    for count in 1..=3 {
        assert_badge(
            &mut tracker,
            &mut store,
            "str_replace",
            &arguments,
            &format!("+{count} -2"),
        );
        arguments.push_str(r"\nnext");
    }
}

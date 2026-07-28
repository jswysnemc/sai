use super::{TranscriptRenderOptions, TranscriptStore};
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::work_status::WorkStatus;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【工作状态测试】构造 transcript 渲染配置。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 完整思考与摘要工具模式的渲染配置
fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

/// 【终端】【工作状态测试】验证实时思考摘要持续推进文字扫光。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn live_reasoning_summary_animates_without_waiting_for_consolidation() {
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: "inspect resize".to_string(),
    });

    let first = store.display_live_tail(80, &options());
    assert!(store.advance_live_animation());
    let second = store.display_live_tail(80, &options());

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first, second);
    assert!(second[0].as_str().contains("tokens"));
}

/// 【终端】【工作状态测试】验证状态替换不会写入历史区。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn work_status_is_replaced_without_becoming_history() {
    let mut store = TranscriptStore::new(100);

    assert!(store.set_work_status(WorkStatus::WaitingResponse));
    let waiting = store.display_live_tail(80, &options());
    assert!(!waiting.is_empty());
    assert!(strip_ansi_for_test(waiting[0].as_str()).contains('s'));

    assert!(store.set_work_status(WorkStatus::Thinking));
    let thinking = store.display_live_tail(80, &options());
    assert!(!thinking.is_empty());
    // 1. 【终端】【工作状态测试】状态切换后只保留 Thinking 与耗时
    let thinking_plain = strip_ansi_for_test(thinking[0].as_str());
    assert!(!thinking_plain.contains(WorkStatus::WaitingResponse.localized_label()));
    assert!(thinking_plain.contains(WorkStatus::Thinking.localized_label()));

    assert!(store.advance_live_animation());
    let animated = store.display_live_tail(80, &options());
    assert!(!animated.is_empty());
    assert!(
        strip_ansi_for_test(animated[0].as_str())
            .contains(WorkStatus::Thinking.localized_label())
    );

    assert!(store.clear_work_status());
    assert!(store.display_live_tail(80, &options()).is_empty());
}

/// 【终端】【工作状态测试】验证工具结果之后继续显示 Working。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn tool_result_keeps_work_status_alive() {
    let mut store = TranscriptStore::new(100);
    assert!(store.set_work_status(WorkStatus::Working));
    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"README.md"}"#.to_string(),
    );
    store.push_tool_result("read_file".to_string(), true, "contents".to_string());

    let live = store.display_live_tail(80, &options());
    assert!(!live.is_empty(), "工具结果后工作动效行不应消失");
    assert!(live.iter().any(|line| {
        strip_ansi_for_test(line.as_str()).contains(WorkStatus::Working.localized_label())
    }));
    assert!(store.advance_live_animation());
}

/// 【终端】【工作状态测试】验证实时思考会隐藏重复工作状态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn work_status_hidden_when_live_reasoning_exists() {
    let mut store = TranscriptStore::new(100);
    assert!(store.set_work_status(WorkStatus::Thinking));
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: "inspect plan".to_string(),
    });

    let live = store.display_live_tail(80, &options());
    let joined = strip_ansi_for_test(
        &live
            .iter()
            .map(|line| line.as_str())
            .collect::<String>(),
    );
    assert!(!joined.contains(WorkStatus::Working.localized_label()));
    assert!(joined.contains("Thinking") || joined.contains("思考"));
}

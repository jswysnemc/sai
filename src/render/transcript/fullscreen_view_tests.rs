use super::*;
use crate::llm::ChatStreamChunk;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::TranscriptMode;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 测试用渲染选项：思考按摘要折叠，与默认前台一致。
fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Summary,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

/// 构造两轮对话：每轮一条用户消息、一段长思考、一段回复。
fn store() -> TranscriptStore {
    let mut store = TranscriptStore::new(10_000);
    for turn in ["first question", "second question"] {
        store.push_user_echo(TranscriptMode::Yolo, turn.to_string());
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: (0..20).map(|i| format!("{turn}-think-{i}\n")).collect(),
        });
        store.finalize_live_tail();
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: format!("{turn} answer"),
        });
        store.finalize_live_tail();
    }
    store
}

/// 文档纯文本，便于断言。
fn plain(document: &FullscreenDocument) -> Vec<String> {
    document
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect()
}

/// 用户消息锚点指向消息所在行，摘要取原文。
#[test]
fn anchors_point_at_user_messages() {
    let mut store = store();
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let text = plain(&document);
    assert_eq!(document.anchors.len(), 2);
    for (anchor, expected) in document
        .anchors
        .iter()
        .zip(["first question", "second question"])
    {
        assert_eq!(anchor.summary, expected);
        assert!(
            text[anchor.row].contains(expected),
            "{:?}",
            text[anchor.row]
        );
    }
    let second = document.anchors[1].row;
    assert_eq!(document.anchor_index_at(second), Some(1));
    assert_eq!(document.anchor_index_at(second - 1), Some(0));
}

/// 每个段落独立展开：展开第一段不影响第二段，行范围随之变化。
#[test]
fn paragraphs_expand_independently() {
    let mut store = store();
    let collapsed = store.render_fullscreen(80, &options(), &HashSet::new());
    let keys = collapsed
        .paragraphs
        .iter()
        .map(|span| span.key)
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 2, "两段思考都应可折叠");
    assert!(collapsed.paragraphs.iter().all(|span| !span.expanded));
    let joined = plain(&collapsed).join("\n");
    assert!(!joined.contains("first question-think-10"), "{joined}");

    let expanded = store.render_fullscreen(80, &options(), &HashSet::from([keys[0]]));
    let joined = plain(&expanded).join("\n");
    assert!(joined.contains("first question-think-10"), "{joined}");
    assert!(!joined.contains("second question-think-10"), "{joined}");
    assert!(expanded.paragraphs[0].expanded && !expanded.paragraphs[1].expanded);
    assert!(expanded.lines.len() > collapsed.lines.len());
    // 展开后后续锚点下移，点击命中仍准确
    assert!(expanded.anchors[1].row > collapsed.anchors[1].row);
    let span = &expanded.paragraphs[1];
    assert_eq!(
        expanded.paragraph_at(span.start).map(|s| s.key),
        Some(span.key)
    );
}

/// 流式思考尾部也能展开收起。
#[test]
fn live_reasoning_is_a_paragraph() {
    let mut store = TranscriptStore::new(10_000);
    store.push_user_echo(TranscriptMode::Yolo, "stream".into());
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..20).map(|i| format!("live-{i}\n")).collect(),
    });
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    assert_eq!(
        document.paragraphs.last().map(|span| span.key),
        Some(LIVE_PARAGRAPH_KEY)
    );
    let open = store.render_fullscreen(80, &options(), &HashSet::from([LIVE_PARAGRAPH_KEY]));
    assert!(plain(&open).join("\n").contains("live-10"));
}

/// 长消息摘要折叠空白并截断。
#[test]
fn summary_is_single_line_and_bounded() {
    let long = format!("a\n  b {}", "x".repeat(300));
    let summary = summarize(&long);
    assert!(summary.starts_with("a b "));
    assert!(summary.ends_with('…'));
    assert_eq!(summary.chars().count(), SUMMARY_CHARS + 1);
}

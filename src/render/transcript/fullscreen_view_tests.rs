use super::*;
use crate::llm::ChatStreamChunk;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::ParagraphPart;
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

/// 构造一条命令很长、输出也很长的 run_command 卡片。
fn command_store() -> TranscriptStore {
    let mut store = TranscriptStore::new(1_000);
    let command = (0..12)
        .map(|index| format!("echo step-{index}"))
        .collect::<Vec<_>>()
        .join(" && \\\n");
    store.push_tool_call(
        "run_command".to_string(),
        serde_json::json!({ "command": command }).to_string(),
    );
    let stdout = (0..30)
        .map(|index| format!("out-{index}\n"))
        .collect::<String>();
    store.push_tool_result(
        "run_command".to_string(),
        true,
        serde_json::json!({ "success": true, "exit_code": 0, "stdout": stdout, "stderr": "" })
            .to_string(),
    );
    store
}

/// 命令卡片拆成命令行与输出两段，两段互不重叠，分界标记不进正文。
#[test]
fn command_card_splits_into_command_and_output_paragraphs() {
    use crate::render::render_expand::{ExpandPart, PART_BOUNDARY};
    let mut store = command_store();
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let keys = document
        .paragraphs
        .iter()
        .map(|span| span.key.part)
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            ParagraphPart::Segment(ExpandPart::Command),
            ParagraphPart::Segment(ExpandPart::Output)
        ]
    );
    let (command, output) = (&document.paragraphs[0], &document.paragraphs[1]);
    assert!(command.end <= output.start, "两段不应重叠");
    let text = plain(&document);
    assert!(text[command.start].contains("$ echo step-0"), "{text:?}");
    assert!(text.iter().all(|line| !line.contains(PART_BOUNDARY)));
}

/// 点击命令行只展开命令，输出仍折叠；反之亦然。
#[test]
fn command_and_output_expand_independently() {
    use crate::render::render_expand::ExpandPart;
    let mut store = command_store();
    let collapsed = plain(&store.render_fullscreen(80, &options(), &HashSet::new())).join("\n");
    assert!(
        !collapsed.contains("step-6") && !collapsed.contains("out-15"),
        "{collapsed}"
    );
    let command_key = ParagraphKey::segment(0, ExpandPart::Command);
    let output_key = ParagraphKey::segment(0, ExpandPart::Output);
    let command_open = store.render_fullscreen(80, &options(), &HashSet::from([command_key]));
    let joined = plain(&command_open).join("\n");
    assert!(joined.contains("step-6"), "命令应展开: {joined}");
    assert!(!joined.contains("out-15"), "输出应保持折叠: {joined}");
    assert!(command_open.paragraphs[0].expanded && !command_open.paragraphs[1].expanded);
    let output_open = store.render_fullscreen(80, &options(), &HashSet::from([output_key]));
    let joined = plain(&output_open).join("\n");
    assert!(joined.contains("out-15"), "输出应展开: {joined}");
    assert!(!joined.contains("step-6"), "命令应保持折叠: {joined}");
}

/// 命令很短时没有可展开的命令段，只登记输出段，点击命令行不会白点。
#[test]
fn short_command_registers_only_the_output_paragraph() {
    use crate::render::render_expand::ExpandPart;
    let mut store = TranscriptStore::new(1_000);
    store.push_shell(
        "ls".to_string(),
        (0..30).map(|index| format!("file-{index}\n")).collect(),
        Some(0),
    );
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let parts = document
        .paragraphs
        .iter()
        .map(|span| span.key.part)
        .collect::<Vec<_>>();
    assert_eq!(parts, vec![ParagraphPart::Segment(ExpandPart::Output)]);
}

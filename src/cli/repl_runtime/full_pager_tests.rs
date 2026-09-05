use super::*;
use crate::cli::repl_input::ReplInputSubmission;
use crate::render;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::TranscriptMode;

/// 全文模式覆盖普通消息、附件、隐藏的思考和超长工具结果，且不受主视图行数限制。
#[test]
fn full_view_preserves_all_sources_beyond_row_cap() {
    let options = TranscriptRenderOptions {
        reasoning_mode: render::ReasoningDisplayMode::Hidden,
        tool_call_mode: render::ToolCallDisplayMode::Hidden,
    };
    let mut runtime = ReplRuntime::new(5, options);
    let mut clipboard = ReplClipboardState::default();
    let mut input = "user opening [ordinary] ".to_string();
    let mut cursor = input.chars().count();
    let pasted = (0..40)
        .map(|i| format!("pasted-line-{i}\n"))
        .collect::<String>();
    clipboard.paste_text_into_input(&mut input, &mut cursor, pasted.clone());
    let submitted = ReplInputSubmission::from_input(AgentMode::Yolo, input, &clipboard);
    clipboard.clear();
    runtime
        .transcript
        .push_user_input(TranscriptMode::Yolo, submitted.echo);
    runtime.transcript.push_chunk(&crate::llm::ChatStreamChunk {
        kind: crate::llm::ChatStreamKind::Reasoning,
        text: (0..20).map(|i| format!("thinking-line-{i}\n")).collect(),
    });
    let payload = (0..100)
        .map(|i| format!("result-line-{i} with enough content to exceed the old payload limit\n"))
        .collect::<String>();
    runtime
        .transcript
        .push_tool_call("custom_tool".into(), "{\"query\":\"all results\"}".into());
    runtime
        .transcript
        .push_tool_result("custom_tool".into(), true, payload.clone());
    runtime.transcript.push_chunk(&crate::llm::ChatStreamChunk {
        kind: crate::llm::ChatStreamKind::Content,
        text: "assistant final answer".into(),
    });
    runtime.transcript.finalize_live_tail();

    let lines = runtime.expanded_transcript_lines(80);
    let text = lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(lines.len() > runtime.transcript.row_cap());
    for expected in [
        "user opening [ordinary]",
        "pasted-line-20",
        "thinking-line-10",
        "result-line-50",
        "result-line-99",
        "assistant final answer",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    assert!(!text.contains("Ctrl+O"));
    assert_eq!(runtime.options, options);

    let blocks = runtime.transcript.expandable_blocks();
    assert!(blocks
        .iter()
        .any(|block| block.body.contains(pasted.trim())));
    assert!(blocks.iter().any(|block| block.body.contains(&payload)));
    let collapsed = layout::display_window(
        &mut runtime.transcript,
        80,
        &options,
        usize::MAX,
        0,
        usize::MAX,
    );
    let text = collapsed
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("Ctrl+O"));
    assert!(!text.contains("pasted-line-20"));
}

/// 分段列表包含当前仍在生成的思考，避免 Ctrl+O 跳回较早的块。
#[test]
fn segment_view_includes_live_reasoning() {
    let mut transcript = TranscriptStore::new(5);
    transcript.push_chunk(&crate::llm::ChatStreamChunk {
        kind: crate::llm::ChatStreamKind::Reasoning,
        text: "current reasoning".to_string(),
    });
    let blocks = transcript.expandable_blocks();
    assert_eq!(blocks.last().unwrap().body, "current reasoning");
}

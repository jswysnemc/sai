use super::*;
use crate::llm::ToolCallStreamProgress;

#[test]
fn tool_status_prefers_running_for_single_active_call() {
    let stats = ToolStats {
        calls: 1,
        ok: 0,
        error: 0,
        progress: None,
    };
    let output = tool_status_text("deep_research", &stats);
    assert!(output.starts_with("deep_research×1 "));
    assert!(output.contains("\x1b[33m"));
    assert!(output.contains("运行中") || output.contains("running"));
}

#[test]
fn tool_status_uses_simple_single_success() {
    let stats = ToolStats {
        calls: 1,
        ok: 1,
        error: 0,
        progress: None,
    };
    let output = tool_status_text("deep_research", &stats);
    assert!(output.starts_with("deep_research×1 "));
    assert!(output.contains("\x1b[32mok\x1b[0m"));
}

#[test]
fn tool_status_counts_mixed_multiple_calls() {
    let stats = ToolStats {
        calls: 3,
        ok: 1,
        error: 1,
        progress: None,
    };
    let output = tool_status_text("grep", &stats);
    assert!(output.starts_with("grep×3 "));
    assert!(output.contains("\x1b[33m"));
    assert!(output.contains("\x1b[32mok\x1b[0m:1"));
    assert!(output.contains("\x1b[31merr\x1b[0m:1"));
}

#[test]
fn summary_styles_distinguish_reasoning_from_tools() {
    assert_eq!(
        style_summary_text("工具", SummaryStyle::Tool),
        "\x1b[2m工具\x1b[0m"
    );
    assert_eq!(
        style_summary_text("思考", SummaryStyle::Reasoning),
        "\x1b[2m\x1b[36m思考\x1b[0m"
    );
}

#[test]
fn tool_event_text_is_append_only_finish_line() {
    let output = tool_event_text("web_search", "ok");
    assert!(output.starts_with("• "));
    assert!(output.contains("web_search"));
    assert!(output.contains("ok"));
}

#[test]
fn read_file_start_status_uses_progress_marker() {
    assert_eq!(tool_start_status("read_file"), "arg");
    assert_eq!(tool_start_status("run_command"), "run");
}

#[test]
fn visible_tool_blocks_do_not_need_extra_start_events() {
    assert!(tool_call_has_visible_block("run_command"));
    assert!(tool_call_has_visible_block("edit_file"));
    assert!(!tool_call_has_visible_block("web_search"));
}

#[test]
fn wait_spinner_detail_line_includes_model_and_thinking_level() {
    let options = StreamRenderOptions {
        readable_tool_names: true,
        wait_model: Some("opencode Zen/gpt-5".to_string()),
        wait_thinking_level: Some("high".to_string()),
    };

    let output = wait_spinner_detail_line(&options).unwrap();

    assert!(output.contains("opencode Zen/gpt-5"));
    assert!(output.contains("high"));
}

#[test]
fn wait_spinner_detail_line_omits_empty_values() {
    let options = StreamRenderOptions {
        readable_tool_names: true,
        wait_model: Some("  ".to_string()),
        wait_thinking_level: None,
    };

    assert!(wait_spinner_detail_line(&options).is_none());
}

#[test]
fn edit_progress_waits_for_renderable_diff_before_consuming_preview() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sample.txt");
    std::fs::write(&path, "old\n").unwrap();
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );

    renderer
        .write_tool_call_progress(&ToolCallStreamProgress {
            index: 0,
            name: Some("edit_file".to_string()),
            arguments_chars: 0,
            arguments_bytes: 0,
            arguments_preview: r#"{"patch":"*** Begin Patch"#.to_string(),
        })
        .unwrap();

    assert!(!renderer.streaming_edit_progress.contains(&0));
    assert_eq!(renderer.pending_streamed_edit_blocks, 0);

    let patch = format!(
        "*** Begin Patch\n*** Update File: {}\n@@\n-old\n+new\n*** End Patch",
        path.display()
    );
    let patch_json = serde_json::to_string(&patch).unwrap();
    let arguments_preview = format!(r#"{{"patch":{patch_json},"path":""#);
    renderer
        .write_tool_call_progress(&ToolCallStreamProgress {
            index: 0,
            name: Some("edit_file".to_string()),
            arguments_chars: arguments_preview.chars().count(),
            arguments_bytes: arguments_preview.len(),
            arguments_preview,
        })
        .unwrap();

    assert!(renderer.streaming_edit_progress.contains(&0));
    assert_eq!(renderer.pending_streamed_edit_blocks, 1);
}

#[test]
fn command_progress_keeps_single_line_status_until_final_call() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );

    renderer
        .write_tool_call_progress(&ToolCallStreamProgress {
            index: 0,
            name: Some("run_command".to_string()),
            arguments_chars: 0,
            arguments_bytes: 0,
            arguments_preview: r#"{"command":"echo"#.to_string(),
        })
        .unwrap();
    // 参数流式期间只保留单行状态，不再做多行命令块预览
    assert!(renderer.live_tool_status.is_active());

    renderer
        .write_tool_call("run_command", r#"{"command":"echo hi"}"#)
        .unwrap();
    // 定稿后一次性输出命令块，单行状态被清除
    assert!(renderer.command_block_tools.contains("run_command"));
    assert!(!renderer.live_tool_status.is_active());
}

#[test]
fn denied_tool_result_is_suppressed_once() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );

    renderer.suppress_denied_result("run_command");
    renderer
        .write_tool_result("run_command", false, "用户拒绝了此工具调用")
        .unwrap();
    // 抑制标记一次性生效
    assert!(!renderer.suppressed_denied_results.contains("run_command"));
}

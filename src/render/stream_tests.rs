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
    let output = tool_status_text("deep_diagnose", &stats);
    assert!(output.starts_with("deep_diagnose×1 "));
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
    let output = tool_status_text("deep_diagnose", &stats);
    assert!(output.starts_with("deep_diagnose×1 "));
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
    assert!(tool_call_has_visible_block("str_replace"));
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

/// 【终端】【CLI 布局】验证流式 Markdown 正文位于视觉引导列右侧。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn streamed_markdown_uses_guide_content_column() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );

    assert!(renderer.render_markdown_delta("partial").is_empty());
    let rendered = renderer.render_markdown_delta(" answer\n");

    assert!(rendered.starts_with("  "), "rendered={rendered:?}");
    assert!(rendered.contains("partial answer"));
}

/// 【终端】【子智能体状态测试】验证推理分片保留物理行边界状态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn subagent_reasoning_tracks_fragmented_line_boundaries() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Full,
        false,
        StreamRenderOptions::default(),
    );

    renderer
        .write_subagent_reasoning("subagent", "partial")
        .unwrap();
    assert!(!renderer.subagent_reasoning_at_line_start);

    renderer.write_subagent_reasoning("subagent", "\n").unwrap();
    assert!(renderer.subagent_reasoning_at_line_start);
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
            name: Some("str_replace".to_string()),
            arguments_chars: 0,
            arguments_bytes: 0,
            arguments_preview: r#"{"path":"partial"#.to_string(),
        })
        .unwrap();

    // 字段尚未闭合时不应产生 diff 预览
    assert!(!renderer.streaming_edit_progress.contains(&0));
    assert_eq!(renderer.pending_streamed_edit_blocks, 0);

    let path_json = serde_json::to_string(&path.display().to_string()).unwrap();
    let arguments_preview =
        format!(r#"{{"path":{path_json},"old_string":"old","new_string":"new","#);
    renderer
        .write_tool_call_progress(&ToolCallStreamProgress {
            index: 0,
            name: Some("str_replace".to_string()),
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

/// 验证正式前台命令调用会清空上一条命令实时预览。
#[test]
fn consecutive_foreground_commands_reset_live_preview() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );
    renderer
        .write_tool_call("run_command", r#"{"command":"printf first"}"#)
        .unwrap();
    let message = crate::tools::command::encode_command_output_for_test(
        crate::tools::command::CommandOutputStream::Stdout,
        b"first\n",
    );
    renderer
        .write_tool_progress("run_command", &message)
        .unwrap();
    assert!(renderer.command_preview.display_texts().0.contains("first"));

    renderer
        .write_tool_call("run_command", r#"{"command":"printf second"}"#)
        .unwrap();

    assert_eq!(
        renderer.command_preview.display_texts(),
        (String::new(), String::new())
    );
}

/// 验证后台命令进度不会进入 CLI 前台命令预览。
#[test]
fn background_progress_uses_generic_tool_rendering() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );
    let message = crate::tools::command::encode_command_output_for_test(
        crate::tools::command::CommandOutputStream::Stdout,
        b"background\n",
    );

    renderer
        .write_tool_progress("background_command", &message)
        .unwrap();

    assert_eq!(
        renderer.command_preview.display_texts(),
        (String::new(), String::new())
    );
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

/// 【终端】【思考流式测试】验证 Full 模式思考正文随增量重绘而非攒到结束。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn full_reasoning_redraws_body_on_each_chunk() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );
    // 逐帧重绘只在真实终端进行；重定向输出时控制序列不生效，会堆叠成重复正文
    let interactive = WaitSpinner::supported();

    // 1. 首个分片到达后正文即进入缓冲；终端环境下同时占用终端行
    renderer
        .write_chunk(ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "第一段思考".to_string(),
        })
        .unwrap();
    assert_eq!(renderer.reasoning_full_buffer, "第一段思考");
    if interactive {
        assert!(
            renderer.reasoning_live_rows > 0,
            "终端环境下首个思考分片应立即渲染"
        );
    } else {
        assert_eq!(
            renderer.reasoning_live_rows, 0,
            "非终端环境不得逐帧重绘，否则输出重复"
        );
    }

    // 2. 后续分片继续累积正文；终端环境下推进动效帧
    let frame_before = renderer.reasoning_frame;
    renderer
        .write_chunk(ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "\n第二段思考".to_string(),
        })
        .unwrap();
    assert!(renderer.reasoning_full_buffer.contains("第二段思考"));
    if interactive {
        assert!(renderer.reasoning_frame > frame_before, "重绘应推进动效帧");
        assert!(renderer.reasoning_live_rows > 0);
    } else {
        assert_eq!(renderer.reasoning_frame, frame_before);
    }

    // 3. 定稿后释放 live 行计数，缓冲清空
    renderer.flush_full_reasoning_block().unwrap();
    assert_eq!(renderer.reasoning_live_rows, 0);
    assert!(renderer.reasoning_full_buffer.is_empty());
    assert_eq!(renderer.reasoning_frame, 0);
    assert!(renderer.reasoning_started.is_none());
}

/// 【终端】【思考流式测试】验证空思考段定稿不会残留 live 行计数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn empty_full_reasoning_flush_resets_live_rows() {
    let mut renderer = StreamRenderer::new(
        ReasoningDisplayMode::Full,
        ToolCallDisplayMode::Summary,
        false,
        StreamRenderOptions::default(),
    );

    renderer.flush_full_reasoning_block().unwrap();

    assert_eq!(renderer.reasoning_live_rows, 0);
    assert_eq!(renderer.reasoning_frame, 0);
    assert!(renderer.mode.is_none());
}

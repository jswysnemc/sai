use super::reflow_state::ReflowState;
use super::viewport::TerminalSize;
use super::ReplRuntime;
use crate::agent::AgentEvent;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::TranscriptRenderOptions;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use crate::runner::{AutomaticInputEvent, AutomaticInputKind, RunnerEvent};
use crossterm::event::Event;

#[test]
fn resize_during_stream_requires_finish_reflow() {
    let mut state = ReflowState::new();
    state.observe(TerminalSize { cols: 80, rows: 24 }, false);
    state.observe(
        TerminalSize {
            cols: 100,
            rows: 24,
        },
        true,
    );

    assert!(state.take_stream_finish_reflow_needed());
    assert!(!state.take_stream_finish_reflow_needed());
}

/// 构造测试渲染选项。
///
/// 参数:
/// - 无
///
/// 返回:
/// - transcript 渲染选项
fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Summary,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

/// 构造正文流式片段事件。
///
/// 参数:
/// - `text`: 正文文本
///
/// 返回:
/// - Runner 事件
fn content_chunk(text: &str) -> RunnerEvent {
    RunnerEvent::Agent(AgentEvent::Chunk(ChatStreamChunk {
        kind: ChatStreamKind::Content,
        text: text.to_string(),
    }))
}

/// 验证完整流式事件序列驱动增量同步管线不出错。
///
/// 覆盖：用户回显、开始事件、长正文流、工具调用与结果、
/// 权限附着与决定、流结束收敛。
#[test]
fn full_stream_event_sequence_drives_reconcile_pipeline() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.record_meta("准备开始".to_string()).unwrap();
    runtime
        .record_user(crate::agent::AgentMode::Audited, "帮我跑测试".to_string())
        .unwrap();
    runtime.record_runner_event(&RunnerEvent::Started).unwrap();

    // 1. 长正文流（超过一屏，验证追加与节流不 panic）
    for index in 0..80 {
        runtime
            .record_runner_event(&content_chunk(&format!("第 {index} 行内容\n")))
            .unwrap();
    }
    // 2. 工具调用生命周期
    runtime
        .record_runner_event(&RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "run_command".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        }))
        .unwrap();
    // 3. 权限附着、选择与决定
    runtime
        .record_permission_request(crate::permission::PermissionRequest {
            id: "perm-1".to_string(),
            session_id: "session".to_string(),
            tool: "run_command".to_string(),
            arguments: r#"{"command":"cargo test"}"#.to_string(),
        })
        .unwrap();
    runtime
        .update_permission_choice("perm-1", crate::render::PermissionChoice::Deny)
        .unwrap();
    runtime
        .resolve_permission(
            "perm-1",
            crate::permission::PermissionDecision::Deny { reply: None },
        )
        .unwrap();
    runtime
        .record_runner_event(&RunnerEvent::Agent(AgentEvent::ToolResult {
            name: "run_command".to_string(),
            ok: false,
            output: "用户拒绝了此工具调用".to_string(),
        }))
        .unwrap();
    // 4. 流结束收敛
    runtime
        .record_runner_event(&RunnerEvent::Completed(crate::llm::ChatResult {
            content: "done".to_string(),
            reasoning: None,
            usage: None,
            tool_calls: Vec::new(),
        }))
        .unwrap();
    runtime.finish_stream().unwrap();

    // 受管行数不变式：屏幕内行数不超过终端高度
    let size = runtime.viewport.size();
    assert!(runtime.stream.on_screen() <= usize::from(size.rows));
}

/// 验证外部输出失步标记后，下一次同步会重启受管区域。
#[test]
fn external_output_restarts_managed_region() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.record_meta("第一段".to_string()).unwrap();
    let before = runtime.stream.on_screen();
    assert!(before > 0);

    runtime.mark_desynced();
    runtime.record_meta("外部输出之后".to_string()).unwrap();

    // 重启后旧行全部视作 scrollback，屏幕上只保留新追加内容
    assert!(runtime.stream.offscreen() >= before);
    assert!(runtime.stream.on_screen() >= 1);
}

/// 验证自动输入事件以蓝色圆点消息写入 TUI 历史。
#[test]
fn automatic_input_event_is_rendered_as_blue_message() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime
        .record_runner_event(&RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
            AutomaticInputKind::ExternalCompletion,
            "后台任务已完成".to_string(),
        )))
        .unwrap();

    let rendered = runtime
        .transcript
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(rendered.contains("\x1b[38;5;39m●"));
    assert!(rendered.contains("后台任务已完成"));
}

#[test]
fn command_progress_renders_five_lines_and_toggles_expansion() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime
        .record_runner_event(&RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "run_command".to_string(),
            arguments: r#"{"command":"test"}"#.to_string(),
        }))
        .unwrap();
    let message = crate::tools::command::encode_command_output_for_test(
        crate::tools::command::CommandOutputStream::Stdout,
        b"one\ntwo\nthree\nfour\nfive\nsix\nseven\n",
    );
    runtime
        .record_runner_event(&RunnerEvent::Agent(AgentEvent::ToolProgress {
            name: "run_command".to_string(),
            message,
        }))
        .unwrap();

    let collapsed = runtime
        .transcript
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(collapsed.contains("Ctrl+O"));

    assert!(runtime.toggle_command_output().unwrap());
    let expanded = runtime
        .transcript
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(expanded.contains("four"));
}

#[test]
fn stream_input_events_are_replayed_in_order() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.queue_input_event(Event::Paste("first".to_string()));
    runtime.queue_input_event(Event::Paste("second".to_string()));

    assert_eq!(
        runtime.pop_input_event(),
        Some(Event::Paste("first".to_string()))
    );
    assert_eq!(
        runtime.pop_input_event(),
        Some(Event::Paste("second".to_string()))
    );
    assert_eq!(runtime.pop_input_event(), None);
}

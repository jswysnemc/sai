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
        .record_user(
            crate::agent::AgentMode::Audited,
            "帮我跑测试".to_string(),
            false,
        )
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
            auto_audit: false,
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
            duration_ms: 0,
            ttft_ms: 0,
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
fn command_progress_renders_head_tail_preview_and_toggles_expansion() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime
        .record_runner_event(&RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "run_command".to_string(),
            arguments: r#"{"command":"test"}"#.to_string(),
        }))
        .unwrap();
    // 超过 2*5 行才会出现中间省略与 Ctrl+O 提示
    let message = crate::tools::command::encode_command_output_for_test(
        crate::tools::command::CommandOutputStream::Stdout,
        b"one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n",
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
    assert!(collapsed.contains("one"));
    assert!(collapsed.contains("twelve"));

    assert!(runtime.toggle_command_output().unwrap());
    let expanded = runtime
        .transcript
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(expanded.contains("six"));
    assert!(expanded.contains("seven"));
    assert!(!expanded.contains("Ctrl+O"));
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

#[test]
fn enqueue_stream_draft_queues_and_clears_input() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().mode = Some(crate::agent::AgentMode::Audited);
    runtime.stream_draft_mut().text = " next task ".to_string();
    runtime.stream_draft_mut().cursor = 5;
    assert!(runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap());
    assert!(runtime.stream_draft().text.is_empty());
    let queued = runtime.take_submission_queue();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].text, "next task");
    assert_eq!(queued[0].mode, crate::agent::AgentMode::Audited);
    assert_eq!(queued[0].insert_at, super::QueueInsertAt::Turn);
}

/// 【TUI】【控制队列】验证运行期间输入的斜杠命令不进消息队列。
///
/// 混进消息队列会被当成提问发给模型，且会连带丢弃其后的排队消息。
#[test]
fn slash_commands_go_to_the_control_queue() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "/tree".to_string();
    assert!(runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap());

    assert!(runtime.take_submission_queue().is_empty());
    assert_eq!(runtime.queued_control_commands(), vec!["/tree".to_string()]);
    assert_eq!(
        runtime.take_next_control_command().as_deref(),
        Some("/tree")
    );
    assert_eq!(runtime.take_next_control_command(), None);
}

/// 【TUI】【控制队列】验证 shell 前缀同样走控制队列。
#[test]
fn shell_prefix_goes_to_the_control_queue() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "!ls -al".to_string();
    assert!(runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap());

    assert!(runtime.take_submission_queue().is_empty());
    assert_eq!(
        runtime.take_next_control_command().as_deref(),
        Some("!ls -al")
    );
}

/// 【TUI】【控制队列】验证普通消息仍进消息队列。
#[test]
fn ordinary_messages_stay_in_the_submission_queue() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "路径是 src/main.rs".to_string();
    assert!(runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap());

    assert!(runtime.queued_control_commands().is_empty());
    assert_eq!(runtime.take_submission_queue().len(), 1);
}

/// 【TUI】【队列编辑】验证撤回把队尾消息交还输入框。
#[test]
fn undo_returns_the_last_queued_message_to_the_input() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "first".to_string();
    runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap();
    runtime.stream_draft_mut().text = "second".to_string();
    runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap();

    assert_eq!(
        runtime.undo_last_queued().unwrap().as_deref(),
        Some("second")
    );
    assert_eq!(runtime.stream_draft().text, "second");
    assert_eq!(runtime.take_submission_queue().len(), 1);
}

/// 【TUI】【队列编辑】验证输入框非空时不覆盖草稿。
#[test]
fn undo_refuses_to_overwrite_a_non_empty_draft() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "queued".to_string();
    runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap();
    runtime.stream_draft_mut().text = "typing".to_string();

    assert_eq!(runtime.undo_last_queued().unwrap(), None);
    assert_eq!(runtime.stream_draft().text, "typing");
    assert_eq!(runtime.take_submission_queue().len(), 1);
}

/// 【TUI】【队列编辑】验证消息队列空时撤回控制命令。
#[test]
fn undo_falls_back_to_the_control_queue() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "/tree".to_string();
    runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap();

    assert_eq!(
        runtime.undo_last_queued().unwrap().as_deref(),
        Some("/tree")
    );
    assert!(runtime.queued_control_commands().is_empty());
}

/// 【TUI】【队列编辑】验证清空同时丢弃消息与控制命令。
#[test]
fn clear_drops_both_queues() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().text = "message".to_string();
    runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap();
    runtime.stream_draft_mut().text = "/tree".to_string();
    runtime
        .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
        .unwrap();

    assert_eq!(runtime.clear_queued().unwrap(), 2);
    assert_eq!(runtime.clear_queued().unwrap(), 0);
    assert!(runtime.take_submission_queue().is_empty());
    assert!(runtime.queued_control_commands().is_empty());
}

/// 入队若干用户消息。
fn enqueue_messages(runtime: &mut ReplRuntime, texts: &[&str]) {
    for text in texts {
        runtime.stream_draft_mut().text = (*text).to_string();
        runtime
            .enqueue_stream_draft(crate::agent::AgentMode::Yolo)
            .unwrap();
    }
}

/// 【TUI】【队列管理】验证 Ctrl+↑ 进入管理并默认高亮队尾。
#[test]
fn ctrl_up_enters_queue_panel_on_the_nearest_item() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["one", "two", "three"]);
    assert!(!runtime.queue_panel_active());
    assert!(runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap());
    assert!(runtime.queue_panel_active());
}

/// 【TUI】【队列管理】验证删除中间项后其余项保持原序。
#[test]
fn queue_panel_deletes_the_selected_item() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["one", "two", "three"]);
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::NONE)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Char('d'), KeyModifiers::NONE)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Char('d'), KeyModifiers::NONE)
        .unwrap();

    let queued = runtime.take_submission_queue();
    assert_eq!(
        queued
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["one", "three"]
    );
}

/// 【TUI】【队列管理】验证输入框非空时拒绝取回编辑。
#[test]
fn queue_panel_edit_refuses_to_overwrite_a_non_empty_draft() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["queued"]);
    runtime.stream_draft_mut().text = "typing".to_string();
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();

    assert_eq!(runtime.stream_draft().text, "typing");
    assert_eq!(runtime.take_submission_queue().len(), 1);
}

/// 【TUI】【队列管理】验证取回编辑把选中项交还输入框。
#[test]
fn queue_panel_edit_restores_the_selected_item() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["one", "two"]);
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Enter, KeyModifiers::NONE)
        .unwrap();

    assert_eq!(runtime.stream_draft().text, "two");
    assert!(!runtime.queue_panel_active());
    let queued = runtime.take_submission_queue();
    assert_eq!(
        queued
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["one"]
    );
}

/// 【TUI】【队列管理】验证立即发送把选中项提到队首。
#[test]
fn queue_panel_send_now_promotes_the_selected_item() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["one", "two", "three"]);
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Char('s'), KeyModifiers::NONE)
        .unwrap();

    let queued = runtime.take_submission_queue();
    assert_eq!(
        queued
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["three", "one", "two"]
    );
    assert_eq!(queued[0].insert_at, super::QueueInsertAt::Request);
    assert_eq!(queued[1].insert_at, super::QueueInsertAt::Turn);
}

/// 【TUI】【队列管理】验证空闲态立即发送取出该项，其余仍留在队列。
#[test]
fn queue_panel_idle_send_now_takes_the_selected_item() {
    use crate::cli::repl_runtime::QueuePanelIdleResult;
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["one", "two"]);
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    let result = runtime
        .handle_queue_panel_idle_key(KeyCode::Char('s'), KeyModifiers::NONE, true)
        .unwrap();
    match result {
        QueuePanelIdleResult::SendNow(item) => assert_eq!(item.text, "two"),
        other => panic!("expected SendNow, got {other:?}"),
    }
    let queued = runtime.take_submission_queue();
    assert_eq!(
        queued
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["one"]
    );
    assert!(!runtime.queue_panel_active());
}

/// 【TUI】【队列管理】验证 Tab 在请求间隔与轮次间隔之间切换。
#[test]
fn queue_panel_tab_toggles_insert_point() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["one"]);
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Tab, KeyModifiers::NONE)
        .unwrap();

    let queued = runtime.take_submission_queue();
    assert_eq!(queued[0].insert_at, super::QueueInsertAt::Request);
}

/// 【TUI】【队列管理】轮次排空留下请求间隔项。
#[test]
fn take_turn_interval_queue_leaves_request_items() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut runtime = ReplRuntime::new(5_000, options());
    enqueue_messages(&mut runtime, &["turn", "request"]);
    runtime
        .handle_queue_panel_key(KeyCode::Up, KeyModifiers::CONTROL)
        .unwrap();
    runtime
        .handle_queue_panel_key(KeyCode::Tab, KeyModifiers::NONE)
        .unwrap();

    let taken = runtime.take_turn_interval_queue();
    assert_eq!(
        taken
            .iter()
            .map(|item| item.text.as_str())
            .collect::<Vec<_>>(),
        ["turn"]
    );
    let remaining = runtime.take_submission_queue();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].text, "request");
    assert_eq!(remaining[0].insert_at, super::QueueInsertAt::Request);
}

#[test]
fn stream_mode_prefers_draft_mode() {
    let mut runtime = ReplRuntime::new(5_000, options());
    runtime.stream_draft_mut().mode = Some(crate::agent::AgentMode::Plan);
    assert_eq!(
        runtime.stream_mode(crate::agent::AgentMode::Yolo),
        crate::agent::AgentMode::Plan
    );
}

/// 构造一条跟随流事件。
///
/// 参数:
/// - `kind`: 事件类型
/// - `payload`: 事件负载
///
/// 返回:
/// - Web 事件
fn follow_event(kind: &str, payload: serde_json::Value) -> crate::web::runs::WebEvent {
    crate::web::runs::WebEvent::new("run-1", "workspace", "session", kind, payload)
}

/// 【跟随模式】远端正文分片实时进入 live tail,不再等轮次结束才落盘。
///
/// 流式渲染只输出到最近一个换行(与持有者本地 live tail 同一语义),
/// 分片带换行符模拟真实模型输出。
#[test]
fn follow_stream_renders_content_deltas_live() {
    let mut runtime = ReplRuntime::new(5_000, options());
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    runtime.follow_remote_stream(rx);

    tx.send(follow_event(
        "message.content.delta",
        serde_json::json!({ "text": "远端流式正文第一行\n" }),
    ))
    .unwrap();
    let progressed = runtime.drain_follow_events().unwrap();
    assert!(progressed, "有事件就应当推进");

    let rendered = runtime
        .transcript
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    assert!(plain.contains("远端流式正文第一行"), "正文应实时可见: {plain}");

    // 轮次结束收敛 live tail,正文仍保留
    tx.send(follow_event("run.completed", serde_json::json!({}))).unwrap();
    runtime.drain_follow_events().unwrap();
    let rendered = runtime
        .transcript
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    assert!(plain.contains("远端流式正文第一行"), "定稿后正文不丢: {plain}");
}

/// 【跟随模式】接入跟随流后读键等待必须周期性唤醒,否则主循环阻塞在读键上,
/// 远端事件要等用户按键才落地。
#[test]
fn follow_stream_wakes_the_idle_tick() {
    let mut runtime = ReplRuntime::new(5_000, options());
    assert!(
        runtime.pending_wait().is_none(),
        "未跟随时无额外唤醒"
    );

    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
    runtime.follow_remote_stream(rx);
    assert!(
        runtime.pending_wait().is_some(),
        "跟随流存在时必须周期性唤醒"
    );

    runtime.stop_following();
    assert!(
        runtime.pending_wait().is_none(),
        "停止跟随后不再唤醒"
    );
}


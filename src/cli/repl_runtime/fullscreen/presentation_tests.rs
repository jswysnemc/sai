use super::*;
use crate::config::TuiStyle;
use crate::render::transcript::{
    AnsiLine, FullscreenDocument, ParagraphKey, ParagraphSpan, TranscriptRenderOptions,
};
use crate::render::{
    activity_animation::strip_ansi_for_test, tui_style, ReasoningDisplayMode, ToolCallDisplayMode,
};
use crate::runner::{AutomaticInputEvent, AutomaticInputKind, RunnerEvent};

/// 【终端】【后续消息测试】目标续轮和排队交付均作为工具通知，不增加导航点。
/// 参数: 无；返回: 无。
#[test]
fn subsequent_messages_render_as_tools_without_navigation_points() {
    for style in [TuiStyle::Classic, TuiStyle::Deepseek] {
        tui_style::set_override(Some(style));
        let options = TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        };
        for event in [
            RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
                AutomaticInputKind::GoalContinuation,
                "Goal 自动续轮：Inspect layout".into(),
            )),
            RunnerEvent::Agent(crate::agent::AgentEvent::InterMessage(
                crate::agent::InterMessageEvent {
                    id: "goal-1".into(),
                    kind: crate::agent::InterMessageKind::GoalContinuation,
                    content: "Goal 自动续轮：Inspect layout".into(),
                },
            )),
            RunnerEvent::Agent(crate::agent::AgentEvent::InterMessage(
                crate::agent::InterMessageEvent {
                    id: "queued-1".into(),
                    kind: crate::agent::InterMessageKind::QueuedUser,
                    content: "FOLLOW-UP-CONTENT".into(),
                },
            )),
        ] {
            let mut runtime = ReplRuntime::new(200, options);
            runtime.transcript.push_user_echo(
                crate::render::transcript::TranscriptMode::Yolo,
                "INITIAL-USER".into(),
            );
            runtime.record_runner_event(&event).unwrap();
            let document = runtime.transcript.render_fullscreen(
                100,
                &options,
                &std::collections::HashSet::new(),
            );
            assert_eq!(document.anchors.len(), 1, "{style:?}: {event:?}");
            let plain = document
                .lines
                .iter()
                .map(|line| strip_ansi_for_test(line.as_str()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                plain.contains("Continuing goal") || plain.contains("Queued message delivered"),
                "{plain}"
            );
            assert!(
                plain.contains("Inspect layout") || plain.contains("FOLLOW-UP-CONTENT"),
                "{plain}"
            );
            runtime.transcript.push_user_echo(
                crate::render::transcript::TranscriptMode::Yolo,
                "NEXT-USER".into(),
            );
            assert_eq!(
                runtime
                    .transcript
                    .render_fullscreen(100, &options, &std::collections::HashSet::new())
                    .anchors
                    .len(),
                2
            );
        }
    }
    tui_style::set_override(None);
}

/// 【终端】【固定状态测试】DeepSeek 等待响应状态进入输入框上方固定面板，不重复出现在会话中。
/// 参数: 无；返回: 无。
#[test]
fn latest_style_waiting_status_is_fixed_above_input() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    let mut runtime = ReplRuntime::new(200, options);
    runtime.enter_fullscreen().unwrap();
    runtime.record_runner_event(&RunnerEvent::Started).unwrap();
    let panel = runtime.bottom_panel_lines(80).join("\n");
    assert!(
        strip_ansi_for_test(&panel).contains("Waiting for response"),
        "{panel:?}"
    );
    let document =
        runtime
            .transcript
            .render_fullscreen(80, &options, &std::collections::HashSet::new());
    assert!(!document
        .lines
        .iter()
        .any(|line| strip_ansi_for_test(line.as_str()).contains("Waiting for response")));
    tui_style::set_override(None);
}

/// 【终端】【悬停按钮测试】悬停折叠标题时替换行首符号，展开状态显示收起按钮，移开后恢复。
/// 参数: 无；返回: 无。
#[test]
fn latest_style_hover_replaces_the_guide_without_moving_text() {
    let layout = FullscreenLayout::compute(80, 24, 5);
    for expanded in [false, true] {
        let mut state = FullscreenState::new();
        state.apply_document(
            FullscreenDocument {
                lines: vec![AnsiLine::new("  \x1b[32m●\x1b[0m Ran command".into())],
                paragraphs: vec![ParagraphSpan {
                    key: ParagraphKey::whole(0),
                    start: 0,
                    end: 1,
                    expanded,
                    control: 0,
                }],
                ..FullscreenDocument::default()
            },
            usize::from(layout.body_height),
        );
        state.pointer_row = Some(0);
        state.pointer_col = Some(2);
        let hovered = paint::compose(&state, &layout);
        let plain = strip_ansi_for_test(&hovered.rows[usize::from(layout.body_top)]);
        assert!(
            plain.trim_start().starts_with(if expanded {
                "▾ Ran command"
            } else {
                "▸ Ran command"
            }),
            "{plain:?}"
        );
        state.pointer_row = None;
        let restored = paint::compose(&state, &layout);
        let normal = strip_ansi_for_test(&restored.rows[usize::from(layout.body_top)]);
        assert!(
            normal.trim_start().starts_with("● Ran command"),
            "{normal:?}"
        );
        assert_eq!(
            unicode_width::UnicodeWidthStr::width(plain.as_str()),
            unicode_width::UnicodeWidthStr::width(normal.as_str())
        );
    }
}

/// 【终端】【后台通知测试】后台完成事件作为工具展示，只保留真实用户消息的导航点。
/// 参数: 无；返回: 无。
#[test]
fn background_completion_does_not_create_a_navigation_point() {
    for style in [TuiStyle::Classic, TuiStyle::Deepseek] {
        tui_style::set_override(Some(style));
        let options = TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        };
        let content = "后台工作已完成，自动继续当前对话\n\n后台命令: echo done (task-1)\n状态: stopped\n说明: No logs attached";
        for event in [
            RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
                AutomaticInputKind::ExternalCompletion,
                content.into(),
            )),
            RunnerEvent::Agent(crate::agent::AgentEvent::InterMessage(
                crate::agent::InterMessageEvent {
                    id: "notice-1".into(),
                    kind: crate::agent::InterMessageKind::ExternalCompletion,
                    content: content.into(),
                },
            )),
        ] {
            let mut runtime = ReplRuntime::new(200, options);
            runtime.transcript.push_user_echo(
                crate::render::transcript::TranscriptMode::Yolo,
                "REAL-USER".into(),
            );
            runtime.record_runner_event(&event).unwrap();
            let document = runtime.transcript.render_fullscreen(
                100,
                &options,
                &std::collections::HashSet::new(),
            );
            assert_eq!(document.anchors.len(), 1);
            let plain = document
                .lines
                .iter()
                .map(|line| strip_ansi_for_test(line.as_str()))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(plain.contains("Background command stopped"), "{plain}");
            assert!(plain.contains("Command: echo done"), "{plain}");
            assert!(!plain.contains("后台工作已完成"), "{plain}");
        }
    }
    tui_style::set_override(None);
}

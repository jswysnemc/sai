use super::*;
use crate::config::TuiMode;
use crate::render::transcript::{
    AnsiLine, FullscreenDocument, ParagraphKey, ParagraphSpan, TranscriptRenderOptions,
};
use crate::render::{
    activity_animation::strip_ansi_for_test, tui_mode, ReasoningDisplayMode, ToolCallDisplayMode,
};
use crate::runner::{AutomaticInputEvent, AutomaticInputKind, RunnerEvent};

/// 【终端】【标题简化】宽窄屏及两种语言只保留会话标题，不再添加右侧快捷键。
/// 参数: 无；返回: 无，快捷键文字重新出现时断言失败。
#[test]
fn header_omits_shortcuts_at_every_width_and_locale() {
    for locale in [crate::i18n::Locale::En, crate::i18n::Locale::Zh] {
        crate::i18n::with_locale(locale, || {
            let mut runtime = runtime_tests::runtime(2);
            for cols in [40, 80, 100, 160] {
                let layout = FullscreenLayout::compute(cols, 30, 4);
                let mut state = FullscreenState::new();
                state.apply_document(
                    runtime.transcript.render_fullscreen(
                        usize::from(layout.content_width),
                        &runtime.options,
                        &Default::default(),
                    ),
                    usize::from(layout.body_height),
                );
                let header = strip_ansi_for_test(&paint::compose(&state, &layout).rows[0]);
                for hint in [
                    "Alt+",
                    "Ctrl+",
                    "Drag to copy",
                    "Click to",
                    "拖动复制",
                    "点击展开",
                ] {
                    assert!(!header.contains(hint), "{header}");
                }
            }
        });
    }
}

/// 【终端】【后续消息测试】目标续轮和排队交付均作为工具通知，不增加导航点。
/// 参数: 无；返回: 无。
#[test]
fn subsequent_messages_render_as_tools_without_navigation_points() {
    for style in [TuiMode::Inline, TuiMode::Fullscreen] {
        tui_mode::set_override(Some(style));
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
    tui_mode::set_override(None);
}

/// 【终端】【固定状态测试】全屏界面等待响应状态进入输入框上方固定面板，不重复出现在会话中。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_waiting_status_is_fixed_above_input() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
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
    tui_mode::set_override(None);
}

/// 【终端】【悬停按钮测试】只在标题首行悬停时切换展开图标，移至正文或移开后恢复。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_hover_preserves_the_guide_without_moving_text() {
    let layout = FullscreenLayout::compute(80, 24, 5);
    for expanded in [false, true] {
        let mut state = FullscreenState::new();
        state.apply_document(
            FullscreenDocument {
                lines: vec![
                    AnsiLine::new("  \x1b[32m●\x1b[0m Ran command".into()),
                    AnsiLine::new("    command output".into()),
                ],
                paragraphs: vec![ParagraphSpan {
                    key: ParagraphKey::whole(0),
                    start: 0,
                    end: 2,
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
        let marker = if expanded { "▾" } else { "▸" };
        assert!(
            plain
                .trim_start()
                .starts_with(&format!("{marker} Ran command")),
            "{plain:?}"
        );
        state.pointer_row = Some(1);
        let body_hover = paint::compose(&state, &layout);
        assert!(
            strip_ansi_for_test(&body_hover.rows[usize::from(layout.body_top)])
                .trim_start()
                .starts_with("● Ran command")
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
    for style in [TuiMode::Inline, TuiMode::Fullscreen] {
        tui_mode::set_override(Some(style));
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
    tui_mode::set_override(None);
}

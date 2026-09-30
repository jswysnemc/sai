use crate::agent::{AgentEvent, AgentMode};
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::{FullscreenEvent, ReplRuntime};
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::TranscriptRenderOptions;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use crate::runner::RunnerEvent;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// 测试用渲染选项。
fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Summary,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

/// 测试用底栏。
fn chrome() -> ReplChrome {
    ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 200_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        status_plugin: None,
    }
}

/// 构造多轮会话：每轮用户消息 + 可折叠的长思考 + 多行回复。
fn runtime(turns: usize) -> ReplRuntime {
    let mut runtime = ReplRuntime::new(10_000, options());
    for turn in 0..turns {
        runtime.transcript.push_user_echo(
            crate::render::transcript::TranscriptMode::Yolo,
            format!("question {turn}"),
        );
        runtime.transcript.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: (0..15)
                .map(|i| format!("turn-{turn}-think-{i}\n"))
                .collect(),
        });
        runtime.transcript.finalize_live_tail();
        runtime.transcript.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: (0..6)
                .map(|i| format!("turn-{turn}-answer-{i}\n\n"))
                .collect(),
        });
        runtime.transcript.finalize_live_tail();
    }
    runtime
}

/// 鼠标事件。
fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

/// 进入全屏后输入框固定在底部，正文区在其上方；退出后恢复主屏布局。
#[test]
fn composer_is_pinned_to_the_bottom() {
    let mut runtime = runtime(3);
    runtime
        .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
        .unwrap();
    runtime.toggle_fullscreen().unwrap();
    let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
    assert_eq!(layout.composer_top + layout.composer_height, 24);
    assert_eq!(runtime.viewport.composer_top(), layout.composer_top);
    // 输入框变化仍走固定底部
    let (top, height) = runtime
        .update_composer(&chrome(), "draft more", 10, false, Vec::new(), 0)
        .unwrap();
    assert_eq!(top + height, 24);
    runtime.toggle_fullscreen().unwrap();
    assert!(runtime.fullscreen.is_none());
}

/// 初始停在底部；滚轮向上离开底部，新输出只提示不跳动。
#[test]
fn wheel_scrolls_and_new_output_is_flagged() {
    let mut runtime = runtime(4);
    runtime.enter_fullscreen().unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let bottom = state.scroll;
    assert!(bottom > 0 && state.follow);
    let outcome = runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 10, 5))
        .unwrap();
    assert_eq!(outcome, FullscreenEvent::Consumed);
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        bottom - 3
    );
    runtime
        .record_runner_event(&RunnerEvent::Agent(AgentEvent::Chunk(ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "fresh output line\n\nmore\n\nmore\n".into(),
        })))
        .unwrap();
    // 流式正文由 live 刷新节拍绘制
    runtime.paint_fullscreen().unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.scroll, bottom - 3, "离开底部后新输出不应拉动视图");
    assert!(state.unseen);
}

/// 点击折叠的思考段展开，再点标题行收起。
#[test]
fn clicking_a_paragraph_toggles_it() {
    let mut runtime = runtime(2);
    runtime.enter_fullscreen().unwrap();
    // 滚到顶部，让第一段思考出现在正文区
    runtime
        .handle_fullscreen_event(&Event::Key(KeyEvent::new(
            KeyCode::Home,
            KeyModifiers::CONTROL,
        )))
        .unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    let span = session.state.document.paragraphs[0].clone();
    let layout = session.state.layout.unwrap();
    let row = layout.body_top + (span.start - session.state.scroll) as u16;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 4, row))
        .unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    assert!(session.state.expanded.contains(&span.key));
    let text = session
        .state
        .document
        .lines
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("turn-0-think-10"), "{text}");
    // 标题行停在原来的屏幕位置，再次点击收起
    let reopened = session.state.document.paragraphs[0].clone();
    assert_eq!(
        layout.body_top + (reopened.start - session.state.scroll) as u16,
        row
    );
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 4, row))
        .unwrap();
    assert!(runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .expanded
        .is_empty());
}

/// 点击概览标记跳到对应用户消息，浮动标题随之切换。
#[test]
fn rail_click_jumps_to_user_message() {
    let mut runtime = runtime(4);
    runtime.enter_fullscreen().unwrap();
    let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
    let rail = layout.rail_col.expect("80 列应显示概览轨道");
    let marks = super::overview::rail_marks(4, usize::from(layout.body_height));
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Down(MouseButton::Left),
            rail,
            layout.body_top + marks[1].row as u16,
        ))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.current_anchor(), Some(1));
    assert!(!state.follow);
    // 悬停标记显示预览卡片
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Moved,
            rail,
            layout.body_top + marks[2].row as u16,
        ))
        .unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    assert_eq!(session.state.hover, Some(2));
    let preview_row = session.state.previous.as_ref().unwrap()
        [usize::from(layout.body_top) + marks[2].row]
        .clone();
    assert!(
        crate::render::activity_animation::strip_ansi_for_test(&preview_row).contains("question 2"),
        "{preview_row}"
    );
}

/// 普通按键不被全屏视图拦截，仍交给输入框。
#[test]
fn typing_passes_through_to_the_composer() {
    let mut runtime = runtime(1);
    runtime.enter_fullscreen().unwrap();
    for key in [
        KeyCode::Char('a'),
        KeyCode::Enter,
        KeyCode::Up,
        KeyCode::Home,
    ] {
        let outcome = runtime
            .handle_fullscreen_event(&Event::Key(KeyEvent::new(key, KeyModifiers::NONE)))
            .unwrap();
        assert_eq!(outcome, FullscreenEvent::Pass, "{key:?}");
    }
    runtime.leave_fullscreen().unwrap();
    let outcome = runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 1, 1))
        .unwrap();
    assert_eq!(outcome, FullscreenEvent::Pass);
}

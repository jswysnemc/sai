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

/// 在指定位置按下并松开左键。
fn click(runtime: &mut ReplRuntime, column: u16, row: u16) {
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), column, row))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Up(MouseButton::Left), column, row))
        .unwrap();
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

/// 点击折叠的思考段展开，再点展开段正文中任意一行收起。
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
    click(&mut runtime, 4, row);
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
    // 标题行停在原来的屏幕位置；点击展开段正文中的一行即可收起
    let reopened = session.state.document.paragraphs[0].clone();
    assert_eq!(
        layout.body_top + (reopened.start - session.state.scroll) as u16,
        row
    );
    let body_row = session
        .state
        .document
        .lines
        .iter()
        .position(|line| line.as_str().contains("turn-0-think-3"))
        .expect("展开后应有思考正文");
    assert!(body_row > reopened.start && body_row < reopened.end);
    let screen_row = layout.body_top + (body_row - session.state.scroll) as u16;
    click(&mut runtime, 4, screen_row);
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

/// 全屏中的折叠提示不再引导按 Ctrl+O，内联视图的提示不受影响。
#[test]
fn fullscreen_fold_hints_do_not_mention_ctrl_o() {
    let mut runtime = ReplRuntime::new(
        10_000,
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        },
    );
    runtime.transcript.push_user_echo(
        crate::render::transcript::TranscriptMode::Yolo,
        "question".into(),
    );
    runtime.transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..30).map(|i| format!("long-think-{i}\n")).collect(),
    });
    runtime.transcript.finalize_live_tail();
    let plain = |lines: &[crate::render::transcript::AnsiLine]| {
        lines
            .iter()
            .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>()
            .join("\n")
    };
    // 1. 内联视图先渲染并写入缓存，提示为 Ctrl+O
    let inline = plain(&runtime.transcript.display_tail(80, &runtime.options));
    assert!(inline.contains("Ctrl+O"), "{inline}");
    // 2. 全屏正文不出现 Ctrl+O，改为点击展开
    runtime.enter_fullscreen().unwrap();
    let screen = plain(&runtime.fullscreen.as_ref().unwrap().state.document.lines);
    assert!(!screen.contains("Ctrl+O"), "{screen}");
    assert!(
        screen.contains("click to expand") || screen.contains("点击展开"),
        "{screen}"
    );
    // 3. 退出后内联视图仍是 Ctrl+O，缓存没有被全屏文案污染
    runtime.leave_fullscreen().unwrap();
    let inline = plain(&runtime.transcript.display_tail(80, &runtime.options));
    assert!(inline.contains("Ctrl+O"), "{inline}");
}

/// 全屏正文保留完整的公式图片放置序列：块级与行内公式都不被按字符宽度截断。
#[test]
fn fullscreen_keeps_formula_image_placements_intact() {
    crate::render::terminal_image::test_override::set(Some(true), Some(false), Some(false));
    let mut runtime = ReplRuntime::new(10_000, options());
    runtime.transcript.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Content,
        text: "行内 $E=mc^2$ 尾巴\n\n$$ e^{i\\pi}+1=0 $$\n\nafter\n".into(),
    });
    runtime.transcript.finalize_live_tail();
    runtime.enter_fullscreen().unwrap();
    crate::render::terminal_image::test_override::set(None, None, None);
    let rows = runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .previous
        .clone()
        .unwrap();
    let placements = rows
        .iter()
        .flat_map(|row| row.match_indices("\x1b_Ga=p").map(move |(at, _)| &row[at..]))
        .collect::<Vec<_>>();
    assert_eq!(placements.len(), 2, "{rows:#?}");
    for placement in placements {
        // 序列必须以 ST 完整闭合
        assert!(placement.contains("\x1b\\"), "{placement:?}");
    }
    let inline_row = rows.iter().find(|row| row.contains("行内")).unwrap();
    assert!(inline_row.contains("尾巴"), "{inline_row:?}");
}


/// 直接拖动即可选中正文并复制，不展开段落；松开后标题提示已复制。
#[test]
fn dragging_selects_and_copies_without_toggling() {
    let mut runtime = runtime(2);
    runtime.enter_fullscreen().unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let answer_row = session
        .state
        .document
        .lines
        .iter()
        .rposition(|line| line.as_str().contains("turn-1-answer-2"))
        .expect("answer line");
    let screen_row = layout.body_top + (answer_row - session.state.scroll) as u16;
    let line = crate::render::activity_animation::strip_ansi_for_test(
        session.state.document.lines[answer_row].as_str(),
    );
    let start = line.find("turn-1").unwrap() as u16;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), start, screen_row))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Drag(MouseButton::Left),
            start + 6,
            screen_row,
        ))
        .unwrap();
    // 拖动中选区以反色绘制
    let painted = runtime.fullscreen.as_ref().unwrap().state.previous.clone().unwrap();
    assert!(painted[usize::from(screen_row)].contains("\x1b[7m"));
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Up(MouseButton::Left),
            start + 6,
            screen_row,
        ))
        .unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    assert_eq!(session.state.copied, Some(6));
    assert!(session.state.expanded.is_empty(), "拖选不应触发展开");
    assert!(session.pending_copy.is_none(), "复制内容应已写出");
    let header = crate::render::activity_animation::strip_ansi_for_test(
        &session.state.previous.as_ref().unwrap()[0],
    );
    assert!(
        header.contains("已复制 6 个字符") || header.contains("Copied 6 characters"),
        "{header}"
    );
    // 下一次按下清掉选区与提示
    click(&mut runtime, start, screen_row);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(state.selection.is_none());
    assert!(state.copied.is_none());
}

/// 拖到正文顶部边缘时向上滚动，选区随之延伸。
#[test]
fn dragging_past_the_top_edge_scrolls() {
    let mut runtime = runtime(4);
    runtime.enter_fullscreen().unwrap();
    let layout = runtime.fullscreen.as_ref().unwrap().state.layout.unwrap();
    let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
    let middle = layout.body_top + layout.body_height / 2;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 2, middle))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Drag(MouseButton::Left),
            2,
            layout.body_top,
        ))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.scroll, before - 1);
    let selection = state.selection.expect("selection");
    assert_eq!(selection.head.row, state.scroll);
}

/// 离开底部后正文末行出现“回到底部”按钮，点击后回到最新输出并隐藏按钮。
#[test]
fn bottom_button_returns_to_latest_output() {
    let mut runtime = runtime(4);
    runtime.enter_fullscreen().unwrap();
    assert!(runtime.fullscreen.as_ref().unwrap().bottom_button.is_none());
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 10, 5))
        .unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    let layout = session.state.layout.unwrap();
    let (start, end) = session.bottom_button.expect("button while scrolled up");
    let last_row = layout.body_top + layout.body_height - 1;
    let painted = crate::render::activity_animation::strip_ansi_for_test(
        &session.state.previous.as_ref().unwrap()[usize::from(last_row)],
    );
    assert!(painted.contains("回到底部") || painted.contains("Back to bottom"), "{painted}");
    click(&mut runtime, (start + end) / 2, last_row);
    let session = runtime.fullscreen.as_ref().unwrap();
    assert!(session.state.follow);
    assert_eq!(session.state.scroll, session.state.max_scroll(usize::from(layout.body_height)));
    assert!(session.bottom_button.is_none());
    assert!(session.state.selection.is_none());
}

use super::runtime_tests::{chrome, mouse, runtime};
use crate::cli::repl_runtime::composer_frame::ComposerFrame;
use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};
use crate::render::activity_animation::strip_ansi_for_test as plain;
use crossterm::event::{MouseButton, MouseEventKind};

/// 【终端】【待办悬停回归】仅标题出现展开箭头，移开恢复且布局不变；无参数和返回值。
#[test]
fn todo_hover_matches_click_target_without_resizing() {
    let mut runtime = runtime(1);
    runtime.transcript.push_tool_result(
        "todo".into(),
        true,
        r#"{"items":[{"text":"current","status":"in_progress"}]}"#.into(),
    );
    runtime
        .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let top = runtime.viewport.composer_top();
    let height = runtime.viewport.composer_height();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, top))
        .unwrap();
    assert_eq!(
        runtime.panel_target_at(4, top),
        Some(crate::cli::repl_runtime::panel_mouse::PanelAction::Todo)
    );
    let signature = runtime.last_composer_signature.as_ref().unwrap();
    assert!(plain(signature.panel_line(0).unwrap())
        .trim_start()
        .starts_with('▸'));
    assert_eq!(runtime.viewport.composer_top(), top);
    assert_eq!(runtime.viewport.composer_height(), height);
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, top + 2))
        .unwrap();
    assert!(plain(
        runtime
            .last_composer_signature
            .as_ref()
            .unwrap()
            .panel_line(0)
            .unwrap()
    )
    .trim_start()
    .starts_with(crate::render::guide_symbols::TODO));
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 4, top))
        .unwrap();
    let expanded_top = runtime.viewport.composer_top();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, expanded_top))
        .unwrap();
    assert!(plain(
        runtime
            .last_composer_signature
            .as_ref()
            .unwrap()
            .panel_line(0)
            .unwrap()
    )
    .trim_start()
    .starts_with('▾'));
}

/// 【终端】【子代理鼠标回归】点击标题展开，点击条目进入会话，返回主会话仍可用；无参数和返回值。
#[test]
fn subagent_panel_hover_and_click_share_visible_rows() {
    use crate::cli::repl_runtime::panel_mouse::PanelAction;
    let mut runtime = runtime(1);
    runtime.transcript.push_tool_call(
        "subagent".into(),
        r#"{"action":"start","description":"task"}"#.into(),
    );
    runtime.transcript.push_tool_result(
        "subagent".into(),
        true,
        r#"{"subagent":{"id":"hover-fixture","status":"completed"}}"#.into(),
    );
    runtime
        .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let top = runtime.viewport.composer_top();
    assert_eq!(
        runtime.panel_target_at(4, top),
        Some(PanelAction::AgentsToggle)
    );
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, top))
        .unwrap();
    assert!(plain(
        runtime
            .last_composer_signature
            .as_ref()
            .unwrap()
            .panel_line(0)
            .unwrap()
    )
    .trim_start()
    .starts_with('▸'));
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 4, top))
        .unwrap();
    assert!(runtime.agent_panel.is_active());
    let top = runtime.viewport.composer_top();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 4, top))
        .unwrap();
    assert!(plain(
        runtime
            .last_composer_signature
            .as_ref()
            .unwrap()
            .panel_line(0)
            .unwrap()
    )
    .trim_start()
    .starts_with('▾'));
    let action = runtime.panel_target_at(8, top + 2).unwrap();
    assert!(matches!(action, PanelAction::Agent(Some(_))));
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 8, top + 2))
        .unwrap();
    assert!(!runtime
        .last_composer_signature
        .as_ref()
        .unwrap()
        .panel_line(2)
        .unwrap()
        .contains('▸'));
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 8, top + 2))
        .unwrap();
    assert_eq!(
        runtime.transcript.viewing_subagent_id(),
        Some("hover-fixture")
    );
    let top = runtime.viewport.composer_top();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 4, top))
        .unwrap();
    let top = runtime.viewport.composer_top();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Down(MouseButton::Left), 8, top + 1))
        .unwrap();
    assert_eq!(runtime.transcript.viewing_subagent_id(), None);
}

/// 【全屏视图】【待办回归】点击可见标题展开和收起，正文条目不触发切换；无参数或返回值。
#[test]
fn todo_header_click_toggles_without_selecting_text() {
    let mut runtime = runtime(2);
    runtime.transcript.push_tool_result("todo".into(), true,
        r#"{"items":[{"text":"current","status":"in_progress"},{"text":"next","status":"pending"}]}"#.into());
    runtime
        .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let compact_top = runtime.viewport.composer_top();
    assert!(runtime.todo_panel_compact);
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Down(MouseButton::Left),
            4,
            compact_top,
        ))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Up(MouseButton::Left),
            4,
            compact_top,
        ))
        .unwrap();
    assert!(!runtime.todo_panel_compact);
    assert!(runtime.viewport.composer_top() < compact_top);
    assert!(runtime.fullscreen.as_ref().unwrap().state.press.is_none());
    assert!(runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .selection
        .is_none());
    let expanded_top = runtime.viewport.composer_top();
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Down(MouseButton::Left),
            4,
            expanded_top + 1,
        ))
        .unwrap();
    assert!(!runtime.todo_panel_compact, "点击条目不应收起面板");
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Down(MouseButton::Left),
            4,
            expanded_top,
        ))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(
            MouseEventKind::Up(MouseButton::Left),
            4,
            expanded_top,
        ))
        .unwrap();
    assert!(runtime.todo_panel_compact);
    assert_eq!(runtime.viewport.composer_top(), compact_top);
    assert!(
        runtime.toggle_todo_panel_compact(),
        "键盘路径仍可切换同一状态"
    );
    assert!(!runtime.todo_panel_compact);
}

/// 【全屏视图】【裁剪回归】面板裁剪或点击行外空白时不命中标题；无参数或返回值。
#[test]
fn clipped_panel_header_has_no_click_target() {
    let mut composer = ComposerFrame::new(chrome(), "draft".into(), 5, false, Vec::new(), 0);
    composer.set_panel_lines(vec!["Todo 0/2".into(), "current".into(), "next".into()]);
    let size = TerminalSize { cols: 40, rows: 24 };
    let full = InlineViewport::fixed(size, 10, 14);
    let (_, signature) = composer.draw_lines(&mut Vec::new(), &full, None).unwrap();
    assert!(signature.hits_panel_header(4, 10));
    assert!(!signature.hits_panel_header(20, 10));
    assert!(!signature.hits_panel_header(4, 11));
    let clipped = InlineViewport::fixed(size, 22, 2);
    let (_, signature) = composer
        .draw_lines(&mut Vec::new(), &clipped, None)
        .unwrap();
    assert!(!signature.hits_panel_header(4, 22));
}

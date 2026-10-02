use super::runtime_tests::{chrome, mouse, runtime};
use crate::cli::repl_runtime::composer_frame::ComposerFrame;
use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};
use crossterm::event::{MouseButton, MouseEventKind};

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

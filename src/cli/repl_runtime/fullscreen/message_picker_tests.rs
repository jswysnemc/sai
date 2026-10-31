use super::message_picker::MessagePicker;
use super::message_picker_view::{overlay, PickerLayout};
use super::runtime_tests::{chrome, mouse, runtime};
use super::{layout::FullscreenLayout, paint, state::FullscreenState};
use crate::cli::repl_runtime::{FullscreenEvent, ReplRuntime};
use crate::render::activity_animation::strip_ansi_for_test;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

/// 【消息导航测试】【键盘事件】发送列表浏览键。
/// 参数: runtime 为运行期，code 为按键；返回: 分流结果。
fn key(runtime: &mut ReplRuntime, code: KeyCode) -> FullscreenEvent {
    runtime
        .handle_fullscreen_event(&Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
        .unwrap()
}

/// 【消息导航测试】【点击事件】发送按下和释放，验证不会误触下方正文。
/// 参数: runtime 为运行期，column/row 为坐标；返回: 无。
fn click(runtime: &mut ReplRuntime, column: u16, row: u16) {
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        assert_eq!(
            runtime
                .handle_fullscreen_event(&mouse(kind, column, row))
                .unwrap(),
            FullscreenEvent::Consumed
        );
    }
}

/// 【消息导航测试】【入口与跳转】点击标题展开完整消息列表，选择后关闭并跳到对应消息。
/// 参数: 无；返回: 无。
#[test]
fn header_opens_all_messages_and_click_jumps() {
    let mut runtime = runtime(4);
    runtime
        .update_composer(&chrome(), "kept draft", 10, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
    click(&mut runtime, 3, 0);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(state.message_picker.active);
    assert_eq!(state.message_picker.selected, 3);
    assert_eq!(state.scroll, before);
    let text = state
        .previous
        .as_ref()
        .unwrap()
        .iter()
        .map(|line| strip_ansi_for_test(line))
        .collect::<Vec<_>>()
        .join("\n");
    for index in 0..4 {
        assert!(
            text.contains(&format!("{}  question {index}", index + 1)),
            "{text}"
        );
    }
    assert!(text.contains("User messages") || text.contains("用户消息"));
    click(&mut runtime, 5, 2);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(!state.message_picker.active);
    assert_eq!(state.current_anchor(), Some(0));
    assert!(state.press.is_none());
    assert!(runtime.fullscreen_active());
}

/// 【消息导航测试】【键盘隔离】方向键与确认进入列表，普通输入和粘贴不修改草稿。
/// 参数: 无；返回: 无。
#[test]
fn keyboard_selects_messages_without_editing_draft() {
    let mut runtime = runtime(12);
    runtime
        .update_composer(&chrome(), "kept draft", 10, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let composer_before = runtime.last_composer_signature.clone();
    click(&mut runtime, 3, 0);
    assert_eq!(key(&mut runtime, KeyCode::Home), FullscreenEvent::Consumed);
    assert_eq!(key(&mut runtime, KeyCode::Down), FullscreenEvent::Consumed);
    assert_eq!(
        key(&mut runtime, KeyCode::Char('x')),
        FullscreenEvent::Consumed
    );
    assert_eq!(
        runtime
            .handle_fullscreen_event(&Event::Paste("do not insert".into()))
            .unwrap(),
        FullscreenEvent::Consumed
    );
    assert!(runtime.last_composer_signature == composer_before);
    assert_eq!(key(&mut runtime, KeyCode::Enter), FullscreenEvent::Consumed);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(!state.message_picker.active);
    assert_eq!(state.current_anchor(), Some(1));
}

/// 【消息导航测试】【取消与滚轮】滚轮只移动列表，取消不改变滚动位置，外部点击不透传。
/// 参数: 无；返回: 无。
#[test]
fn cancel_and_outside_click_keep_body_unchanged() {
    let mut runtime = runtime(12);
    runtime.enter_fullscreen().unwrap();
    click(&mut runtime, 3, 0);
    let scroll = runtime.fullscreen.as_ref().unwrap().state.scroll;
    key(&mut runtime, KeyCode::Home);
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 3, 3))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.message_picker.selected, 1);
    assert_eq!(state.scroll, scroll);
    key(&mut runtime, KeyCode::Esc);
    assert!(
        !runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .state
            .message_picker
            .active
    );
    click(&mut runtime, 3, 0);
    let expanded = runtime.fullscreen.as_ref().unwrap().state.expanded.clone();
    click(&mut runtime, 3, 16);
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert!(!state.message_picker.active);
    assert_eq!(state.expanded, expanded);
    assert_eq!(state.scroll, scroll);
    assert!(state.selection.is_none());
}

/// 【消息导航测试】【长列表】所有消息均可分页访问，缩放后焦点保持可见，空会话不能展开。
/// 参数: 无；返回: 无。
#[test]
fn long_list_keeps_selection_visible_at_all_sizes() {
    let mut picker = MessagePicker::default();
    assert!(!picker.open(None, 0, 8));
    assert!(picker.open(Some(90), 100, 8));
    picker.move_by(-8, 100, 8);
    assert_eq!(picker.selected, 82);
    picker.select(99, 100, 8);
    assert_eq!(picker.start, 92);
    for visible in [1, 3, 8, 2] {
        picker.sync(100, visible);
        assert!(picker.selected >= picker.start && picker.selected < picker.start + visible);
    }
    picker.sync(2, 1);
    assert_eq!(picker.selected, 1);
    picker.sync(0, 8);
    assert!(!picker.active);
}

/// 【消息导航测试】【屏幕边界】窄屏和矮屏不超出正文，中文摘要按显示列数截断。
/// 参数: 无；返回: 无。
#[test]
fn panel_fits_narrow_and_short_screens_and_ignores_hint_rows() {
    let mut state = FullscreenState::new();
    let mut source = runtime(15);
    source.enter_fullscreen().unwrap();
    state.document = source.fullscreen.as_ref().unwrap().state.document.clone();
    state.document.anchors[0].summary = "中文消息".repeat(60);
    for (cols, height) in [(8, 2), (20, 4), (40, 12), (100, 24)] {
        let layout = FullscreenLayout::compute(cols, height, 4);
        let geometry = PickerLayout::compute(&layout, state.document.anchors.len());
        state.message_picker.open(Some(0), 15, geometry.visible);
        let mut rows = vec![String::new(); usize::from(layout.composer_top)];
        overlay(&state, &layout, &mut rows);
        for row in &rows {
            assert!(
                crate::cli::repl_text::visible_width(row) <= usize::from(cols),
                "{row:?}"
            );
        }
        let hit_rows = (0..layout.composer_top)
            .filter(|row| {
                geometry
                    .item_at(2, *row, state.message_picker.start)
                    .is_some()
            })
            .count();
        assert_eq!(hit_rows, geometry.visible);
        assert!(geometry.item_at(2, layout.composer_top, 0).is_none());
        assert!(geometry.item_at(layout.cols, layout.body_top, 0).is_none());
    }
}

/// 【消息导航测试】【入口边界】会话开头也能打开列表，右侧快捷键区域不作为入口。
/// 参数: 无；返回: 无。
#[test]
fn header_hitbox_matches_drawn_title_not_shortcuts() {
    let mut runtime = runtime(2);
    runtime.enter_fullscreen().unwrap();
    let state = &mut runtime.fullscreen.as_mut().unwrap().state;
    state.scroll = 0;
    let layout = FullscreenLayout::compute(120, 30, 4);
    assert!(paint::header_target_at(state, &layout, 3, 0));
    assert!(!paint::header_target_at(state, &layout, 119, 0));
    assert!(!paint::header_target_at(state, &layout, 3, 1));
    state.document.anchors.clear();
    assert!(!paint::header_target_at(state, &layout, 3, 0));
}

/// 【消息导航测试】【全局事件】中断退出仍交给原入口，释放按键不会重复跳转。
/// 参数: 无；返回: 无。
#[test]
fn global_shortcuts_close_panel_before_passing_to_input() {
    let mut runtime = runtime(2);
    runtime.enter_fullscreen().unwrap();
    click(&mut runtime, 3, 0);
    let mut release = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    release.kind = crossterm::event::KeyEventKind::Release;
    assert_eq!(
        runtime
            .handle_fullscreen_event(&Event::Key(release))
            .unwrap(),
        FullscreenEvent::Consumed
    );
    assert!(
        runtime
            .fullscreen
            .as_ref()
            .unwrap()
            .state
            .message_picker
            .active
    );
    assert_eq!(
        runtime
            .handle_fullscreen_event(&Event::Resize(42, 18))
            .unwrap(),
        FullscreenEvent::Pass
    );
    for code in [KeyCode::Char('c'), KeyCode::Char('o')] {
        let result = runtime
            .handle_fullscreen_event(&Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL)))
            .unwrap();
        assert_eq!(result, FullscreenEvent::Pass);
        let state = &runtime.fullscreen.as_ref().unwrap().state;
        assert!(!state.message_picker.active);
        let text = state.previous.as_ref().unwrap().join("\n");
        assert!(!text.contains("User messages") && !text.contains("用户消息"));
        click(&mut runtime, 3, 0);
    }
}

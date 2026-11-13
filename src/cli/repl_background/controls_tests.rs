use super::{controls, model::TaskManager, panel_rect::PanelRect, service::Request, view};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

/// 【后台任务】【确认点击】命中区域来自实际页脚，危险操作固定原请求，不改变选中目标
#[test]
fn confirmation_buttons_follow_rendered_geometry() {
    let mut manager = TaskManager::default();
    manager.confirm = Some(Request::Cleanup);
    let frame = view::render(
        &mut manager,
        PanelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        },
    );
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 3,
        row: 23,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(controls::hit(&manager, mouse, &frame), Some(KeyCode::Enter));
    assert_eq!(
        super::input::mouse(&mut manager, mouse, &frame),
        super::input::Effect::Request(Request::Cleanup)
    );
}

/// 【后台任务】【筛选点击】筛选标签使用实际列宽，空白区域不会误触发操作
#[test]
fn filter_tabs_are_clickable() {
    let mut manager = TaskManager::default();
    let frame = view::render(
        &mut manager,
        PanelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        },
    );
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 3,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    assert_eq!(
        controls::hit(&manager, mouse, &frame),
        Some(KeyCode::Char('1'))
    );
    assert_eq!(
        controls::hit(
            &manager,
            MouseEvent {
                column: 98,
                ..mouse
            },
            &frame
        ),
        None
    );
}

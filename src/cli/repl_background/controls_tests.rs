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

/// 【后台任务】【分页顶栏点击】点击通道胶囊和跟随开关触发切换动作。
#[test]
fn pager_header_stream_and_follow_pills_are_clickable() {
    use super::model_tests::task;
    use super::service::Update;

    let mut manager = TaskManager::default();
    manager.apply(Update {
        tasks: vec![task("t1", "running")],
        output: Some((
            "t1".into(),
            Ok(super::model::Output {
                stdout: Some("line1\nline2".into()),
                ..Default::default()
            }),
        )),
        notice: None,
    });
    manager.pager = true;
    manager.follow = true;
    let frame = view::render(
        &mut manager,
        PanelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        },
    );
    let stream_hit = frame
        .buttons
        .iter()
        .find(|hit| hit.row == 1 && hit.key == KeyCode::Tab)
        .copied()
        .expect("stream pill hitbox");
    let follow_hit = frame
        .buttons
        .iter()
        .find(|hit| hit.row == 1 && hit.key == KeyCode::Char('f'))
        .copied()
        .expect("follow pill hitbox");

    assert_eq!(
        controls::hit(
            &manager,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: stream_hit.start_col,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
            &frame,
        ),
        Some(KeyCode::Tab)
    );
    assert_eq!(
        controls::hit(
            &manager,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: follow_hit.start_col,
                row: 1,
                modifiers: KeyModifiers::NONE,
            },
            &frame,
        ),
        Some(KeyCode::Char('f'))
    );
}

/// 【后台任务】【右栏滚动】列表模式下在右侧输出区滚轮滚动不会改变左侧选中的任务。
#[test]
fn right_output_pane_wheel_scrolls_tail_independently() {
    use super::model_tests::task;
    use super::service::Update;

    let mut manager = TaskManager::default();
    let long_stdout = (1..=30)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    manager.apply(Update {
        tasks: vec![task("t1", "running"), task("t2", "finished")],
        output: Some((
            "t1".into(),
            Ok(super::model::Output {
                stdout: Some(long_stdout),
                ..Default::default()
            }),
        )),
        notice: None,
    });
    let frame = view::render(
        &mut manager,
        PanelRect {
            x: 0,
            y: 0,
            width: 110,
            height: 24,
        },
    );
    let tail_rect = frame.tail_rect.expect("split mode exposes tail_rect");
    super::input::mouse(
        &mut manager,
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: tail_rect.x,
            row: tail_rect.y,
            modifiers: KeyModifiers::NONE,
        },
        &frame,
    );
    assert_eq!(manager.selected_id.as_deref(), Some("t1"));
    assert_eq!(manager.tail_offset, 3);
    super::input::mouse(
        &mut manager,
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: tail_rect.x,
            row: tail_rect.y,
            modifiers: KeyModifiers::NONE,
        },
        &frame,
    );
    assert_eq!(manager.tail_offset, 0);
}

/// 【后台任务】【按钮悬停】悬停到可用控制胶囊时，对应位置应用柔和悬停提亮，离开后清除。
#[test]
fn hovered_control_pill_highlights_only_when_pointer_enters() {
    let mut manager = TaskManager::default();
    let baseline = view::render(
        &mut manager,
        PanelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        },
    );
    let filter_btn = baseline
        .buttons
        .iter()
        .find(|hit| hit.row == 1 && hit.key == KeyCode::Char('2'))
        .copied()
        .expect("filter pill button");
    super::input::mouse(
        &mut manager,
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: filter_btn.start_col,
            row: filter_btn.row,
            modifiers: KeyModifiers::NONE,
        },
        &baseline,
    );
    assert_eq!(
        manager.hovered_button,
        Some((filter_btn.start_col, filter_btn.row))
    );
    let hovered = view::render(
        &mut manager,
        PanelRect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        },
    );
    assert_ne!(hovered.lines[1], baseline.lines[1]);

    super::input::mouse(
        &mut manager,
        MouseEvent {
            kind: MouseEventKind::Moved,
            column: 90,
            row: 5,
            modifiers: KeyModifiers::NONE,
        },
        &hovered,
    );
    assert_eq!(manager.hovered_button, None);
}

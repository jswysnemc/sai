//! 面板、日志和确认状态的键盘与滚轮操作。

use super::{
    model::{Filter, TaskManager},
    service::Request,
    view::TaskFrame,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Effect {
    None,
    Exit,
    Request(Request),
}

/// 【后台任务】【按键处理】按视图状态分发操作，危险操作固定目标后再确认。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `key`: 键盘输入事件
/// - `rows`: 日志或任务可见行数
///
/// 返回:
/// - 外部异步或退出动作
pub(super) fn handle(manager: &mut TaskManager, key: KeyEvent, rows: usize) -> Effect {
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c' | 'd'))
    {
        return Effect::Exit;
    }
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        return Effect::None;
    }
    if manager.confirm.is_some() {
        return match key.code {
            KeyCode::Enter | KeyCode::Char('y' | 'Y') => {
                Effect::Request(manager.confirm.take().unwrap())
            }
            KeyCode::Esc | KeyCode::Char('n' | 'N' | 'q') => {
                manager.confirm = None;
                Effect::None
            }
            _ => Effect::None,
        };
    }
    if manager.pager {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => manager.pager = false,
            KeyCode::Up | KeyCode::Char('k') => manager.scroll(-1, rows),
            KeyCode::Down | KeyCode::Char('j') => manager.scroll(1, rows),
            KeyCode::PageUp | KeyCode::Char('b') => manager.scroll(-(rows as isize), rows),
            KeyCode::PageDown | KeyCode::Char(' ') => manager.scroll(rows as isize, rows),
            KeyCode::Home | KeyCode::Char('g') => {
                manager.follow = false;
                manager.offset = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                manager.follow = false;
                manager.offset = manager.log_lines().len().saturating_sub(rows);
            }
            KeyCode::Char('f') => {
                manager.offset = manager.pager_offset(rows);
                manager.follow = !manager.follow;
            }
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('v') => manager.cycle_stream(),
            KeyCode::Char('r') => return Effect::Request(Request::Refresh),
            _ => {}
        }
        return Effect::None;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => return Effect::Exit,
        KeyCode::Up | KeyCode::Char('k') => manager.move_selection(-1),
        KeyCode::Down | KeyCode::Char('j') => manager.move_selection(1),
        KeyCode::PageUp => manager.move_selection(-(rows as isize)),
        KeyCode::PageDown => manager.move_selection(rows as isize),
        KeyCode::Home => manager.move_selection(isize::MIN),
        KeyCode::End => manager.move_selection(isize::MAX),
        KeyCode::Enter | KeyCode::Char('o') if manager.selected().is_some() => {
            manager.pager = true;
            manager.follow = true;
        }
        KeyCode::Tab | KeyCode::BackTab => manager.set_filter(match manager.filter {
            Filter::All => Filter::Running,
            Filter::Running => Filter::Finished,
            Filter::Finished => Filter::All,
        }),
        KeyCode::Char('1') => manager.set_filter(Filter::All),
        KeyCode::Char('2') => manager.set_filter(Filter::Running),
        KeyCode::Char('3') => manager.set_filter(Filter::Finished),
        KeyCode::Char('v') => manager.cycle_stream(),
        KeyCode::Char('r') => {
            manager.notice = None;
            return Effect::Request(Request::Refresh);
        }
        KeyCode::Char('s' | 'f') if !manager.busy => {
            if let Some(task) = manager.selected().filter(|task| task.status == "running") {
                manager.confirm = Some(Request::Stop {
                    id: task.id.clone(),
                    force: key.code == KeyCode::Char('f'),
                });
            }
        }
        KeyCode::Char('x')
            if !manager.busy && manager.tasks.iter().any(|task| task.status != "running") =>
        {
            manager.confirm = Some(Request::Cleanup)
        }
        _ => {}
    }
    Effect::None
}

/// 【后台任务】【鼠标操作】支持筛选栏、通道胶囊、任务行、确认按钮点击及区域内滚轮。
///
/// 参数:
/// - `manager`: 任务管理状态
/// - `mouse`: 鼠标输入事件
/// - `frame`: 已渲染帧几何
///
/// 返回:
/// - 外部异步或退出动作
pub(super) fn mouse(manager: &mut TaskManager, mouse: MouseEvent, frame: &TaskFrame) -> Effect {
    let inside = frame.rect.contains(mouse.column, mouse.row);
    if inside && mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        if let Some(key) = super::controls::hit(manager, mouse, frame) {
            return handle(
                manager,
                KeyEvent::new(key, KeyModifiers::NONE),
                frame.page_rows,
            );
        }
    }
    if matches!(mouse.kind, MouseEventKind::Moved) {
        // 1. 【后台任务】【悬停反馈】同步记录任务行与可点击按钮悬停状态，离开后清除
        manager.hovered = frame
            .targets
            .iter()
            .find(|(row, _)| inside && *row == mouse.row)
            .map(|(_, id)| id.clone());
        manager.hovered_button =
            super::controls::hit(manager, mouse, frame).map(|_| (mouse.column, mouse.row));
        return Effect::None;
    }
    if manager.confirm.is_some() || !inside {
        return Effect::None;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
            let delta = if mouse.kind == MouseEventKind::ScrollUp {
                -3
            } else {
                3
            };
            if manager.pager {
                manager.scroll(delta, frame.page_rows);
            } else if frame
                .tail_rect
                .is_some_and(|rect| rect.contains(mouse.column, mouse.row))
                && frame.tail_rows > 0
            {
                manager.scroll_tail(delta, frame.tail_rows);
            } else {
                manager.move_selection(delta);
            }
        }
        MouseEventKind::Down(MouseButton::Left) if !manager.pager => {
            if let Some((_, id)) = frame.targets.iter().find(|(row, _)| *row == mouse.row) {
                if manager.selected_id.as_deref() == Some(id.as_str()) {
                    // 2. 【后台任务】【快捷打开】再次单击已选中条目时直接进入全屏日志视图
                    manager.pager = true;
                    manager.follow = true;
                } else {
                    manager.selected_id = Some(id.clone());
                    manager.offset = 0;
                    manager.follow = true;
                }
            }
        }
        _ => {}
    }
    Effect::None
}

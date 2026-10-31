//! 合并尚未处理的连续拖动事件，绘制最新位置而不是逐帧重放旧位置。

use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use std::io;

const MAX_DRAG_EVENTS: usize = 512;

/// 【终端侧栏】【拖动合并】只跨过同一左键拖动流，松开、按键和滚轮保持原有顺序。
/// 参数: first 为首个拖动事件，next 为非阻塞读取器；返回: 最新拖动及未处理的下一事件。
fn collect_drag(
    first: Event,
    mut next: impl FnMut() -> io::Result<Option<Event>>,
) -> io::Result<(Event, Option<Event>)> {
    let mut latest = first;
    for _ in 1..MAX_DRAG_EVENTS {
        let Some(event) = next()? else { break };
        match &event {
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Drag(MouseButton::Left) => {
                latest = event
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Up(MouseButton::Left) => {
                return Ok((event, None))
            }
            _ => return Ok((latest, Some(event))),
        }
    }
    Ok((latest, None))
}

impl ReplRuntime {
    /// 【终端侧栏】【拖动调度】已捕获分界线时才预读鼠标，普通文本选择和输入不参与合并。
    /// 参数: event 为当前事件；返回: 合并后的事件，未参与合并时为空，读取错误向调用者返回。
    pub(super) fn coalesce_sidebar_drag(&mut self, event: &Event) -> io::Result<Option<Event>> {
        if !self.sidebar_control.dragging
            || !matches!(event, Event::Mouse(mouse) if mouse.kind == MouseEventKind::Drag(MouseButton::Left))
        {
            return Ok(None);
        }
        let (latest, pending) = collect_drag(event.clone(), || {
            #[cfg(test)]
            {
                Ok(self.pop_input_event())
            }
            #[cfg(not(test))]
            {
                crate::cli::repl_input::event_batch::next_ready_event(self)
            }
        })?;
        if let Some(event) = pending {
            self.return_input_event(event);
        }
        Ok(Some(latest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::repl_runtime::fullscreen::runtime_tests::mouse;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::collections::VecDeque;

    /// 【拖动测试】【事件顺序】密集拖动只留下最新位置，按键、滚轮与松开事件不丢失。
    /// 参数: 无；返回: 无。
    #[test]
    fn dense_drag_batches_keep_latest_position_and_stop_at_control_events() {
        let first = mouse(MouseEventKind::Drag(MouseButton::Left), 70, 5);
        let key = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        let mut events = (71..130)
            .map(|col| mouse(MouseEventKind::Drag(MouseButton::Left), col, 5))
            .collect::<VecDeque<_>>();
        events.push_back(key.clone());
        let (latest, pending) = collect_drag(first.clone(), || Ok(events.pop_front())).unwrap();
        assert_eq!(
            latest,
            mouse(MouseEventKind::Drag(MouseButton::Left), 129, 5)
        );
        assert_eq!(pending, Some(key));
        let up = mouse(MouseEventKind::Up(MouseButton::Left), 96, 5);
        let wheel = mouse(MouseEventKind::ScrollUp, 96, 5);
        let mut events = VecDeque::from([up.clone(), wheel.clone()]);
        assert_eq!(
            collect_drag(first, || Ok(events.pop_front())).unwrap(),
            (up, None)
        );
        assert_eq!(events.pop_front(), Some(wheel));
    }

    /// 【拖动测试】【读取上限】连续鼠标流有处理上限，给模型输出和快捷键保留执行机会。
    /// 参数: 无；返回: 无。
    #[test]
    fn drag_collection_has_a_bounded_read_budget() {
        let first = mouse(MouseEventKind::Drag(MouseButton::Left), 70, 5);
        let mut count = 0;
        collect_drag(first.clone(), || {
            count += 1;
            Ok(Some(first.clone()))
        })
        .unwrap();
        assert_eq!(count, MAX_DRAG_EVENTS - 1);
    }
}

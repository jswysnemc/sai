//! 【会话选择】【鼠标导航】保存绘制时的可见行和按钮，避免分组标题造成索引偏移。

use super::model::Picker;
use crate::cli::mouse_menu::{ButtonRow, ClickTracker};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Item(usize),
    Key(KeyCode),
}

#[derive(Default)]
pub(super) struct Mouse {
    pub(super) hits: Vec<(u16, Range<usize>, Target)>,
    click: ClickTracker<Target>,
}

impl Mouse {
    /// 【会话选择】【事件转换】参数为事件与选择状态；返回键盘等价动作，候选点击仅选择。
    pub(super) fn event(&mut self, event: Event, picker: &mut Picker) -> Event {
        let Event::Mouse(mouse) = event else {
            self.click.clear();
            return event;
        };
        let target = self
            .hits
            .iter()
            .find(|(row, columns, _)| {
                *row == mouse.row && columns.contains(&usize::from(mouse.column))
            })
            .map(|(_, _, action)| *action);
        if let Some(target) = self.click.update(mouse, target) {
            match target {
                Target::Item(index) => picker.selected = index,
                Target::Key(code) => return Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            }
        }
        if matches!(target, Some(Target::Item(_))) {
            match mouse.kind {
                MouseEventKind::ScrollUp => picker.selected = picker.selected.saturating_sub(1),
                MouseEventKind::ScrollDown => {
                    picker.selected =
                        (picker.selected + 1).min(picker.visible.len().saturating_sub(1))
                }
                _ => {}
            }
        }
        Event::Mouse(mouse)
    }
}

/// 【会话选择】【操作按钮】参数为可用宽度；返回恢复、取消和范围切换的可见按钮。
pub(super) fn buttons(width: usize) -> ButtonRow<Target> {
    use crate::i18n::text as t;
    ButtonRow::new(
        [
            (t("Resume", "恢复").into(), Target::Key(KeyCode::Enter)),
            (t("Cancel", "取消").into(), Target::Key(KeyCode::Esc)),
            (t("Scope", "范围").into(), Target::Key(KeyCode::Tab)),
        ],
        width,
    )
}

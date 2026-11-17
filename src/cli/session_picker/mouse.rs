//! 【会话选择】【鼠标导航】共享可见行、按钮与列表框矩形，支持二次点击确认与全列表滚轮滚动。

use super::model::Picker;
use crate::cli::mouse_menu::{ButtonRow, ClickTracker};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Target {
    Item(usize),
    Key(KeyCode),
}

/// 一帧绘制产出的点击与滚轮几何区域。
#[derive(Default, Clone)]
pub(super) struct DrawHits {
    pub(super) hits: Vec<(u16, Range<usize>, Target)>,
    pub(super) list_rows: Range<u16>,
    pub(super) list_cols: Range<u16>,
}

#[derive(Default)]
pub(super) struct Mouse {
    pub(super) state: DrawHits,
    pub(super) click: ClickTracker<Target>,
}

impl Mouse {
    /// 【会话选择】【事件转换】将鼠标单击、二次点击确认与框内滚轮转换为选择更新或键盘动作。
    ///
    /// 参数:
    /// - `event`: 终端事件
    /// - `picker`: 会话选择状态
    ///
    /// 返回:
    /// - 供主循环继续处理的事件
    pub(super) fn event(&mut self, event: Event, picker: &mut Picker) -> Event {
        let Event::Mouse(mouse) = event else {
            self.click.clear();
            return event;
        };
        let target = self
            .state
            .hits
            .iter()
            .find(|(row, columns, _)| {
                *row == mouse.row && columns.contains(&usize::from(mouse.column))
            })
            .map(|(_, _, action)| *action);

        // 1. 【会话选择】【二次点击恢复】首次点击移动焦点，再次点击已选中会话直接确认恢复
        if let Some(target) = self.click.update(mouse, target) {
            match target {
                Target::Item(index) => {
                    if picker.selected == index {
                        return Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                    }
                    picker.selected = index;
                }
                Target::Key(code) => return Event::Key(KeyEvent::new(code, KeyModifiers::NONE)),
            }
        }

        // 2. 【会话选择】【框内滚轮】只要指针位于列表矩形内，悬停分组标题或空白行均正常滚动
        if self.state.list_rows.contains(&mouse.row)
            && self.state.list_cols.contains(&mouse.column)
        {
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

/// 【会话选择】【操作按钮】返回恢复、取消与范围切换按钮及底栏快捷键说明。
///
/// 参数:
/// - `width`: 底栏可用宽度
///
/// 返回:
/// - 带快捷键提示的可点击按钮行
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
    .with_hint(
        t(
            "Enter resume · Tab scope · Esc cancel",
            "Enter 恢复 · Tab 范围 · Esc 取消",
        ),
        width,
    )
}

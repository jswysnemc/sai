//! 【终端】【菜单鼠标】共享单击判定与紧凑按钮行的绘制、命中坐标。

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

pub(super) struct ClickTracker<T> {
    pressed: Option<(u16, u16, T)>,
}

impl<T> Default for ClickTracker<T> {
    /// 【终端】【单击状态】无参数；返回未按下的鼠标状态。
    fn default() -> Self {
        Self { pressed: None }
    }
}

impl<T: Copy + Eq> ClickTracker<T> {
    /// 【终端】【单击判定】参数为鼠标事件与当前目标；仅同点释放且未拖动时返回动作。
    pub(super) fn update(&mut self, mouse: MouseEvent, target: Option<T>) -> Option<T> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.pressed = target.map(|target| (mouse.column, mouse.row, target));
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let (column, row, pressed) = self.pressed.take()?;
                if column == mouse.column && row == mouse.row && Some(pressed) == target {
                    return Some(pressed);
                }
            }
            MouseEventKind::Moved => {}
            _ => self.pressed = None,
        }
        None
    }

    /// 【终端】【单击清理】布局或焦点变化后取消旧按下状态；无参数和返回值。
    pub(super) fn clear(&mut self) {
        self.pressed = None;
    }
}

pub(super) struct ButtonRow<T> {
    pub(super) text: String,
    hits: Vec<(Range<usize>, T)>,
}

impl<T: Copy> ButtonRow<T> {
    /// 【终端】【按钮提示】参数为快捷键说明与行宽；追加可见提示而不改变按钮范围，返回当前行。
    pub(super) fn with_hint(mut self, hint: &str, width: usize) -> Self {
        let remaining = width.saturating_sub(UnicodeWidthStr::width(self.text.as_str()) + 2);
        if remaining > 0 {
            self.text.push_str("  ");
            self.text
                .push_str(&crate::render::clip_to_width(hint, remaining, ""));
        }
        self
    }
    /// 【终端】【按钮布局】参数为标签、动作及可用列数；返回完整可见按钮与显示列命中区。
    pub(super) fn new(buttons: impl IntoIterator<Item = (String, T)>, width: usize) -> Self {
        let mut result = Self {
            text: String::new(),
            hits: Vec::new(),
        };
        for (label, action) in buttons {
            let gap = usize::from(!result.text.is_empty());
            let start = UnicodeWidthStr::width(result.text.as_str()) + gap;
            let text = format!("[{label}]");
            let end = start + UnicodeWidthStr::width(text.as_str());
            if end > width {
                break;
            }
            if gap > 0 {
                result.text.push(' ');
            }
            result.text.push_str(&text);
            result.hits.push((start..end, action));
        }
        result
    }

    /// 【终端】【按钮命中】参数为相对显示列；返回完整可见按钮的动作。
    pub(super) fn hit(&self, column: u16) -> Option<T> {
        self.hits
            .iter()
            .find(|(range, _)| range.contains(&usize::from(column)))
            .map(|(_, action)| *action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    /// 【终端】【鼠标回归】参数为事件类型及坐标；返回合成鼠标事件。
    fn event(kind: MouseEventKind, column: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row: 2,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// 【终端】【拖选保护】无参数和返回值；拖回起点、异点释放和布局清理均不可触发按钮。
    #[test]
    fn only_stationary_release_activates_a_target() {
        let mut click = ClickTracker::default();
        let down = event(MouseEventKind::Down(MouseButton::Left), 3);
        let up = event(MouseEventKind::Up(MouseButton::Left), 3);
        assert_eq!(click.update(down, Some(1)), None);
        assert_eq!(click.update(up, Some(1)), Some(1));
        for cancel in [
            event(MouseEventKind::Drag(MouseButton::Left), 3),
            event(MouseEventKind::Up(MouseButton::Left), 4),
        ] {
            click.update(down, Some(1));
            assert_eq!(click.update(cancel, Some(1)), None);
            assert_eq!(click.update(up, Some(1)), None);
        }
        click.update(down, Some(1));
        click.clear();
        assert_eq!(click.update(up, Some(1)), None);
    }

    /// 【终端】【中文按钮】无参数和返回值；宽字符按显示列命中，窄屏隐藏按钮不产生动作。
    #[test]
    fn unicode_buttons_share_visible_bounds() {
        let buttons = ButtonRow::new([("保存".into(), 1), ("取消".into(), 2)], 12);
        assert_eq!(buttons.text, "[保存]");
        assert_eq!(buttons.hit(5), Some(1));
        assert_eq!(buttons.hit(6), None);
        assert_eq!(buttons.hit(10), None);
    }
}

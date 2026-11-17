//! 【终端】【菜单鼠标】共享单击判定与紧凑按钮行的绘制、命中坐标。

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

pub(crate) struct ClickTracker<T> {
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
    pub(crate) fn update(&mut self, mouse: MouseEvent, target: Option<T>) -> Option<T> {
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
    pub(crate) fn clear(&mut self) {
        self.pressed = None;
    }
}

pub(crate) struct ButtonRow<T> {
    pub(crate) text: String,
    hits: Vec<(Range<usize>, T)>,
    items: Vec<(String, T)>,
    hint_suffix: String,
}

impl<T: Copy> ButtonRow<T> {
    /// 【终端】【按钮提示】参数为快捷键说明与行宽；追加可见提示而不改变按钮范围，返回当前行。
    ///
    /// 参数:
    /// - `hint`: 快捷键说明文本
    /// - `width`: 当前行最大显示列宽
    ///
    /// 返回:
    /// - 追加提示后的按钮行
    pub(crate) fn with_hint(mut self, hint: &str, width: usize) -> Self {
        let remaining = width.saturating_sub(UnicodeWidthStr::width(self.text.as_str()) + 2);
        if remaining > 0 {
            let suffix = format!("  {}", crate::render::clip_to_width(hint, remaining, ""));
            self.text.push_str(&suffix);
            self.hint_suffix = suffix;
        }
        self
    }

    /// 【终端】【按钮布局】参数为标签、动作及可用列数；返回完整可见按钮与显示列命中区。
    ///
    /// 参数:
    /// - `buttons`: 按钮标签与关联动作序列
    /// - `width`: 当前行最大显示列宽
    ///
    /// 返回:
    /// - 包含纯文本与列命中区的按钮行
    pub(crate) fn new(buttons: impl IntoIterator<Item = (String, T)>, width: usize) -> Self {
        // 1. 【终端】【按钮排布】按显示宽度依次排入完整按钮并记录命中区间
        let mut result = Self {
            text: String::new(),
            hits: Vec::new(),
            items: Vec::new(),
            hint_suffix: String::new(),
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
            result.items.push((text, action));
        }
        result
    }

    /// 【终端】【按钮命中】参数为相对显示列；返回完整可见按钮的动作。
    ///
    /// 参数:
    /// - `column`: 相对行首的终端显示列坐标
    ///
    /// 返回:
    /// - 命中的按钮动作，留白或越界返回空
    pub(crate) fn hit(&self, column: u16) -> Option<T> {
        self.hits
            .iter()
            .find(|(range, _)| range.contains(&usize::from(column)))
            .map(|(_, action)| *action)
    }

    /// 【终端】【命中区间读取】返回全部可见按钮的显示列区间与对应动作。
    ///
    /// 参数:
    /// - `self`: 按钮行实例
    ///
    /// 返回:
    /// - 按钮列范围与动作切片
    #[cfg(test)]
    pub(crate) fn hits(&self) -> &[(Range<usize>, T)] {
        &self.hits
    }

    /// 【终端】【按钮着色渲染】按每个按钮的独立样式渲染行文本，保持与纯文本完全一致的显示列坐标。
    ///
    /// 参数:
    /// - `button_style`: 根据按钮动作返回 ANSI 样式前缀的闭包
    /// - `hint_style`: 尾部快捷键提示文本的 ANSI 样式前缀
    ///
    /// 返回:
    /// - 带 ANSI 样式且列宽与 `self.text` 一致的文本行
    pub(crate) fn render_styled(
        &self,
        mut button_style: impl FnMut(T) -> String,
        hint_style: &str,
    ) -> String {
        // 1. 【终端】【按钮着色】逐个渲染可见按钮，按钮之间保留无背景色的单格空白
        let mut rendered = String::new();
        for (index, (label, action)) in self.items.iter().enumerate() {
            if index > 0 {
                rendered.push(' ');
            }
            let style = button_style(*action);
            rendered.push_str(&style);
            rendered.push_str(label);
            rendered.push_str("\x1b[0m");
        }
        // 2. 【终端】【提示追加】尾部说明使用独立弱化样式，不改变任何按钮的列命中范围
        if !self.hint_suffix.is_empty() {
            rendered.push_str(hint_style);
            rendered.push_str(&self.hint_suffix);
            rendered.push_str("\x1b[0m");
        }
        rendered
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

    /// 【终端】【着色按钮】验证逐按钮着色后去色纯文本与命中范围保持严格一致。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn styled_buttons_preserve_plain_text_and_hit_columns() {
        let buttons = ButtonRow::new([("保存".into(), 1), ("取消".into(), 2)], 40)
            .with_hint("Enter 保存", 40);
        let styled = buttons.render_styled(
            |action| {
                if action == 2 {
                    "\x1b[48;2;40;44;52m\x1b[38;2;220;220;220m".into()
                } else {
                    "\x1b[38;2;103;193;207m".into()
                }
            },
            "\x1b[38;2;137;141;145m",
        );
        let plain = crate::render::ansi_clip::strip_ansi(&styled);
        assert_eq!(plain, buttons.text);
        assert_eq!(buttons.hits(), &[(0..6, 1), (7..13, 2)]);
        assert!(styled.contains("\x1b[48;2;40;44;52m\x1b[38;2;220;220;220m[取消]\x1b[0m"));
    }
}

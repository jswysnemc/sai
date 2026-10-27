use super::composer_frame::InputGeometry;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::ops::Range;

/// 【终端】【输入选区】源码字节区间独立于面板高度和软折行位置。
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct ComposerSelection {
    source: String,
    anchor: usize,
    head: usize,
    pressed: bool,
    dragged: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    /// 【终端】【鼠标测试】参数为事件类型和屏幕坐标；返回测试事件。
    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    /// 【终端】【拖选回归】正反向选择都复制相同文本，单击不复制；无参数和返回值。
    #[test]
    fn selection_copies_in_both_directions_without_mutating_input() {
        let geometry = InputGeometry::new("alpha beta", 30, 0, 10, 3, 1);
        for (start, end) in [(3, 8), (8, 3)] {
            let mut selection = ComposerSelection::default();
            assert!(
                selection
                    .handle(
                        mouse(MouseEventKind::Down(MouseButton::Left), start, 10),
                        Some(&geometry)
                    )
                    .handled
            );
            assert!(
                selection
                    .handle(
                        mouse(MouseEventKind::Drag(MouseButton::Left), end, 10),
                        Some(&geometry)
                    )
                    .changed
            );
            let result = selection.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), end, 10),
                Some(&geometry),
            );
            assert_eq!(result.copy.as_deref(), Some("alpha"));
            assert_eq!(geometry.text, "alpha beta");
            assert!(selection.clear());
            selection.handle(
                mouse(MouseEventKind::Down(MouseButton::Left), start, 10),
                Some(&geometry),
            );
            assert!(selection
                .handle(
                    mouse(MouseEventKind::Up(MouseButton::Left), start, 10),
                    Some(&geometry)
                )
                .copy
                .is_none());
        }
    }
}

#[derive(Default)]
pub(super) struct MouseEffect {
    pub(super) handled: bool,
    pub(super) changed: bool,
    pub(super) copy: Option<String>,
}

impl ComposerSelection {
    /// 【终端】【选区清理】输入或操作焦点变化时移除旧选区。
    /// 参数: 无；返回: 是否需要清理屏幕高亮。
    pub(super) fn clear(&mut self) -> bool {
        let changed = self.anchor != self.head;
        *self = Self::default();
        changed
    }

    /// 【终端】【输入范围】仅在草稿仍与选择时相同时返回区间，避免高亮转移到新输入。
    /// 参数: `geometry` 为当前输入映射；返回: 非空源码选区。
    pub(super) fn range(&self, geometry: &InputGeometry) -> Option<Range<usize>> {
        (self.source == geometry.text && self.anchor != self.head)
            .then_some(self.anchor.min(self.head)..self.anchor.max(self.head))
    }

    /// 【终端】【输入拖选】按下记录锚点，拖动高亮，释放复制；不改写输入和光标。
    /// 参数: `mouse` 为鼠标事件，`geometry` 为最近绘制的输入映射；返回: 消费状态、重绘需求及待复制文本。
    pub(super) fn handle(
        &mut self,
        mouse: MouseEvent,
        geometry: Option<&InputGeometry>,
    ) -> MouseEffect {
        let Some(geometry) = geometry else {
            return MouseEffect::default();
        };
        if self.source != geometry.text {
            self.clear();
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let changed = self.clear();
                if let Some(position) = geometry.hit(mouse.column, mouse.row, false) {
                    self.source = geometry.text.clone();
                    self.anchor = position;
                    self.head = position;
                    self.pressed = true;
                    MouseEffect {
                        handled: true,
                        changed,
                        copy: None,
                    }
                } else {
                    MouseEffect {
                        changed,
                        ..MouseEffect::default()
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) if self.pressed => {
                let before = self.head;
                self.head = geometry
                    .hit(mouse.column, mouse.row, true)
                    .unwrap_or(self.head);
                self.dragged = true;
                MouseEffect {
                    handled: true,
                    changed: before != self.head,
                    copy: None,
                }
            }
            MouseEventKind::Up(MouseButton::Left) if self.pressed => {
                self.pressed = false;
                if self.dragged {
                    self.head = geometry
                        .hit(mouse.column, mouse.row, true)
                        .unwrap_or(self.head);
                }
                let copy = self
                    .range(geometry)
                    .map(|range| self.source[range].to_string());
                MouseEffect {
                    handled: true,
                    changed: true,
                    copy,
                }
            }
            _ => MouseEffect::default(),
        }
    }
}

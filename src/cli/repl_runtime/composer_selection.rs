use super::composer_frame::InputGeometry;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use std::ops::Range;

/// 【终端】【输入选区】源码字节区间独立于面板高度和软折行位置。
#[derive(Clone, Default, PartialEq, Eq)]
pub(super) struct ComposerSelection {
    target: SelectionTarget,
    source: String,
    anchor: usize,
    head: usize,
    pressed: bool,
    dragged: bool,
    pending_cursor: Option<usize>,
}

/// 【终端】【选区目标】输入源码和底栏快照分开匹配，避免相同文字导致高亮串区。
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum SelectionTarget {
    #[default]
    Input,
    Footer,
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

    /// 【终端】【底栏拖选回归】两行底栏正反向拖选均复制纯文本，输入区域不出现高亮。
    /// 参数：无；返回：无，跨行复制内容错误或选区串入输入时断言失败。
    #[test]
    fn footer_selection_copies_both_rows_and_stays_separate_from_input() {
        let input = InputGeometry::new("yolo test", 40, 0, 10, 3, 1);
        let footer = InputGeometry::from_rendered_lines(
            &[
                "   \x1b[32myolo test\x1b[0m  ".into(),
                "   Enter send".into(),
            ],
            40,
            12,
        );
        for (start, end) in [((3, 12), (8, 13)), ((8, 13), (3, 12))] {
            let mut selection = ComposerSelection::default();
            assert!(
                selection
                    .handle(
                        mouse(MouseEventKind::Down(MouseButton::Left), start.0, start.1),
                        Some(&input),
                        Some(&footer)
                    )
                    .handled
            );
            assert!(
                selection
                    .handle(
                        mouse(MouseEventKind::Drag(MouseButton::Left), end.0, end.1),
                        Some(&input),
                        Some(&footer)
                    )
                    .changed
            );
            let result = selection.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), end.0, end.1),
                Some(&input),
                Some(&footer),
            );
            assert_eq!(result.copy.as_deref(), Some("yolo test\n   Enter"));
            assert!(selection.range(&input).is_none());
            assert!(selection.footer_range(&footer).is_some());
            assert_eq!(input.text, "yolo test");
        }
    }

    /// 【终端】【底栏更新回归】状态内容变化或底栏被隐藏时清除旧选区，不复制过期文本。
    /// 参数：无；返回：无，旧状态被复制或选区未清除时断言失败。
    #[test]
    fn footer_changes_invalidate_pending_copy() {
        let footer = InputGeometry::from_rendered_lines(&["yolo".into()], 40, 12);
        let changed = InputGeometry::from_rendered_lines(&["plan".into()], 40, 12);
        for current in [Some(&changed), None] {
            let mut selection = ComposerSelection::default();
            selection.handle(
                mouse(MouseEventKind::Down(MouseButton::Left), 0, 12),
                None,
                Some(&footer),
            );
            selection.handle(
                mouse(MouseEventKind::Drag(MouseButton::Left), 4, 12),
                None,
                Some(&footer),
            );
            let result = selection.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), 4, 12),
                None,
                current,
            );
            assert!(result.changed);
            assert!(result.copy.is_none());
            assert!(selection.footer_range(&footer).is_none());
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
                        Some(&geometry),
                        None,
                    )
                    .handled
            );
            assert!(
                selection
                    .handle(
                        mouse(MouseEventKind::Drag(MouseButton::Left), end, 10),
                        Some(&geometry),
                        None,
                    )
                    .changed
            );
            let result = selection.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), end, 10),
                Some(&geometry),
                None,
            );
            assert_eq!(result.copy.as_deref(), Some("alpha"));
            assert_eq!(geometry.text, "alpha beta");
            assert!(selection.clear());
            selection.handle(
                mouse(MouseEventKind::Down(MouseButton::Left), start, 10),
                Some(&geometry),
                None,
            );
            assert!(selection
                .handle(
                    mouse(MouseEventKind::Up(MouseButton::Left), start, 10),
                    Some(&geometry),
                    None,
                )
                .copy
                .is_none());
        }
    }

    /// 【终端】【单击定位】中文命中转换为字符索引，行尾空白定位到行尾，拖动不产生光标请求。
    /// 参数: 无；返回: 无。
    #[test]
    fn clicks_request_character_positions_but_drag_keeps_cursor_unchanged() {
        let input = "中文 alpha\n\nend";
        let geometry = InputGeometry::new(input, 30, 0, 10, 3, 3);
        for (column, row, expected) in
            [(5, 10, 1), (6, 10, 1), (20, 10, 8), (3, 11, 9), (8, 12, 13)]
        {
            let mut selection = ComposerSelection::default();
            selection.handle(
                mouse(MouseEventKind::Down(MouseButton::Left), column, row),
                Some(&geometry),
                None,
            );
            let result = selection.handle(
                mouse(MouseEventKind::Up(MouseButton::Left), column, row),
                Some(&geometry),
                None,
            );
            assert!(result.copy.is_none());
            assert_eq!(selection.take_cursor(input), Some(expected));
            assert_eq!(selection.take_cursor(input), None);
        }
        let mut selection = ComposerSelection::default();
        selection.handle(
            mouse(MouseEventKind::Down(MouseButton::Left), 3, 10),
            Some(&geometry),
            None,
        );
        selection.handle(
            mouse(MouseEventKind::Drag(MouseButton::Left), 8, 10),
            Some(&geometry),
            None,
        );
        let result = selection.handle(
            mouse(MouseEventKind::Up(MouseButton::Left), 8, 10),
            Some(&geometry),
            None,
        );
        assert!(result.copy.is_some());
        assert_eq!(selection.take_cursor(input), None);
    }

    /// 【终端】【旧点击隔离】草稿变化或点击底栏后不移动输入光标。
    /// 参数: 无；返回: 无。
    #[test]
    fn old_text_and_footer_clicks_do_not_request_input_cursor() {
        let geometry = InputGeometry::new("draft", 30, 0, 10, 3, 1);
        let mut selection = ComposerSelection::default();
        selection.handle(
            mouse(MouseEventKind::Down(MouseButton::Left), 4, 10),
            Some(&geometry),
            None,
        );
        selection.handle(
            mouse(MouseEventKind::Up(MouseButton::Left), 4, 10),
            Some(&geometry),
            None,
        );
        assert_eq!(selection.take_cursor("new draft"), None);
        let footer = InputGeometry::from_rendered_lines(&["footer".into()], 30, 13);
        selection.handle(
            mouse(MouseEventKind::Down(MouseButton::Left), 2, 13),
            Some(&geometry),
            Some(&footer),
        );
        selection.handle(
            mouse(MouseEventKind::Up(MouseButton::Left), 2, 13),
            Some(&geometry),
            Some(&footer),
        );
        assert_eq!(selection.take_cursor("draft"), None);
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
        (self.target == SelectionTarget::Input
            && self.source == geometry.text
            && self.anchor != self.head)
            .then_some(self.anchor.min(self.head)..self.anchor.max(self.head))
    }

    /// 【终端】【底栏范围】仅对选择时的底栏文本返回区间，状态变化后不高亮其他文字。
    /// 参数：geometry 为当前底栏映射；返回：非空 UTF-8 字节选区。
    pub(super) fn footer_range(&self, geometry: &InputGeometry) -> Option<Range<usize>> {
        (self.target == SelectionTarget::Footer
            && self.source == geometry.text
            && self.anchor != self.head)
            .then_some(self.anchor.min(self.head)..self.anchor.max(self.head))
    }

    /// 【终端】【输入拖选】按下记录锚点，拖动高亮并复制，单击记录待应用光标。
    /// 参数: `mouse` 为鼠标事件，`geometry/footer` 为最近绘制的输入与底栏映射；返回: 消费状态、重绘需求及待复制文本。
    pub(super) fn handle(
        &mut self,
        mouse: MouseEvent,
        geometry: Option<&InputGeometry>,
        footer: Option<&InputGeometry>,
    ) -> MouseEffect {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            let changed = self.clear();
            let hit = geometry
                .and_then(|geometry| {
                    geometry
                        .cursor_hit(mouse.column, mouse.row)
                        .map(|position| (SelectionTarget::Input, geometry, position))
                })
                .or_else(|| {
                    footer.and_then(|geometry| {
                        geometry
                            .hit(mouse.column, mouse.row, false)
                            .map(|position| (SelectionTarget::Footer, geometry, position))
                    })
                });
            if let Some((target, geometry, position)) = hit {
                self.target = target;
                self.source = geometry.text.clone();
                self.anchor = position;
                self.head = position;
                self.pressed = true;
                return MouseEffect {
                    handled: true,
                    changed,
                    copy: None,
                };
            }
            return MouseEffect {
                changed,
                ..MouseEffect::default()
            };
        }
        let geometry = match self.target {
            SelectionTarget::Input => geometry,
            SelectionTarget::Footer => footer,
        };
        let Some(geometry) = geometry else {
            return MouseEffect {
                changed: self.clear(),
                ..MouseEffect::default()
            };
        };
        if self.source != geometry.text {
            return MouseEffect {
                changed: self.clear(),
                ..MouseEffect::default()
            };
        }
        match mouse.kind {
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
                let copy = (self.anchor != self.head).then(|| {
                    self.source[self.anchor.min(self.head)..self.anchor.max(self.head)].to_string()
                });
                if !self.dragged && self.target == SelectionTarget::Input {
                    self.pending_cursor = Some(self.source[..self.anchor].chars().count());
                }
                MouseEffect {
                    handled: true,
                    changed: true,
                    copy,
                }
            }
            _ => MouseEffect::default(),
        }
    }

    /// 【终端】【光标请求】仅当前草稿仍匹配鼠标按下快照时返回位置，旧请求读取后丢弃。
    /// 参数: input 为当前草稿；返回: 字符偏移，拖选或文本已变化时为空。
    pub(super) fn take_cursor(&mut self, input: &str) -> Option<usize> {
        let cursor = self.pending_cursor.take()?;
        (self.source == input).then_some(cursor)
    }
}

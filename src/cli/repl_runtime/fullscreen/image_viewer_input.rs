//! 图片点击在正常正文选区释放后判定，浮窗打开期间优先隔离输入。

use super::{image_viewer_layout::ViewerLayout, input::Effect};
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

impl ReplRuntime {
    /// 【图片浮窗】【输入分流】处理图片点击、关闭和悬停，不把浮窗操作发送到聊天框。
    /// 参数: event 为终端事件；返回: 已处理效果，不属于图片交互时为空。
    pub(super) fn image_viewer_event(&mut self, event: &Event) -> Option<Effect> {
        if let Some(effect) = self.composer_image_event(event) {
            return Some(effect);
        }
        let session = self.fullscreen.as_mut()?;
        let layout = session.state.layout?;
        if session.image_viewer.source.is_none() {
            if session.state.message_picker.active || self.queue_panel.is_active() {
                return None;
            }
            let Event::Mouse(mouse) = event else {
                return None;
            };
            if mouse.kind != MouseEventKind::Up(MouseButton::Left)
                || !layout.in_body(mouse.row)
                || mouse.column >= layout.content_width
                || session.state.selection.is_some()
            {
                return None;
            }
            let row = session.state.scroll + usize::from(mouse.row - layout.body_top);
            let press = session.state.press?;
            if press.row != row || press.col != usize::from(mouse.column) {
                return None;
            }
            let image = session.state.document.lines.get(row)?.image.as_ref()?;
            let column = usize::from(mouse.column);
            if column < image.column || column >= image.column + image.width {
                return None;
            }
            session.image_viewer.open(image.source.clone());
            session.state.press = None;
            session.state.pointer_row = None;
            session.state.pointer_col = None;
            session.state.previous = None;
            return Some(Effect::Repaint);
        }
        let geometry = ViewerLayout::compute(&layout);
        let mut close = false;
        let mut changed = false;
        match event {
            Event::Resize(..) => {
                session.image_viewer.dismiss_press = None;
                return Some(Effect::Pass);
            }
            Event::Key(key) if key.kind == KeyEventKind::Release => {}
            Event::Key(key)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'o')) =>
            {
                session.image_viewer.close();
                session.state.previous = None;
                return Some(Effect::Pass);
            }
            Event::Key(key) => close = key.code == KeyCode::Esc,
            Event::Mouse(mouse) => {
                let hovered = geometry.close_at(mouse.column, mouse.row);
                match mouse.kind {
                    MouseEventKind::Moved => {
                        changed = session.image_viewer.close_hovered != hovered;
                        session.image_viewer.close_hovered = hovered;
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        let inside = session
                            .image_viewer
                            .image_area
                            .is_some_and(|area| area.contains(mouse.column, mouse.row));
                        session.image_viewer.dismiss_press =
                            (!inside).then_some((mouse.column, mouse.row));
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        session.image_viewer.dismiss_press = None;
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        close = session.image_viewer.dismiss_press.take()
                            == Some((mouse.column, mouse.row))
                            && !session
                                .image_viewer
                                .image_area
                                .is_some_and(|area| area.contains(mouse.column, mouse.row));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        if close {
            session.image_viewer.close();
            session.state.previous = None;
            changed = true;
        }
        Some(if changed {
            Effect::Repaint
        } else {
            Effect::Nothing
        })
    }

    /// 【图片浮窗】【光标隔离】查询浮窗是否打开，输入区重绘后不显示编辑光标。
    /// 参数: 无；返回: 是否正在查看大图。
    pub(in crate::cli::repl_runtime) fn image_viewer_active(&self) -> bool {
        self.fullscreen
            .as_ref()
            .is_some_and(|session| session.image_viewer.source.is_some())
    }
}

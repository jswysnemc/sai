//! 输入区缩略图复用正文图片浮窗，点击依据最后一帧的可见图片判定。

use super::input::Effect;
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use std::sync::Arc;

pub(super) struct ComposerImagePress {
    source: Arc<str>,
    column: u16,
    row: u16,
    cancelled: bool,
}

impl ReplRuntime {
    /// 【输入图片】【点击分流】按下和释放必须命中同一张可见缩略图，拖动或缩放取消点击。
    /// 参数: event 为终端事件；返回: 图片交互效果，其他事件交给原输入路径。
    pub(super) fn composer_image_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        if session.image_viewer.source.is_some()
            || session.state.message_picker.active
            || self.queue_panel.is_active()
        {
            session.image_viewer.thumbnail_press = None;
            return None;
        }
        let Event::Mouse(mouse) = event else {
            if let Some(press) = session.image_viewer.thumbnail_press.as_mut() {
                press.cancelled = true;
            }
            return None;
        };
        let source = self
            .last_composer_signature
            .as_ref()
            .and_then(|signature| signature.image_source_at(mouse.column, mouse.row));
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                session.image_viewer.thumbnail_press = source.map(|source| ComposerImagePress {
                    source: source.clone(),
                    column: mouse.column,
                    row: mouse.row,
                    cancelled: false,
                });
                session.image_viewer.thumbnail_press.as_ref()?;
                self.composer_selection.clear();
                session.state.selection = None;
                session.state.press = None;
                session.state.pointer_row = None;
                session.state.pointer_col = None;
                Some(Effect::Repaint)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                let press = session.image_viewer.thumbnail_press.take()?;
                if !press.cancelled
                    && press.column == mouse.column
                    && press.row == mouse.row
                    && source == Some(&press.source)
                {
                    session.image_viewer.open(press.source);
                    session.state.previous = None;
                    Some(Effect::Repaint)
                } else {
                    Some(Effect::Nothing)
                }
            }
            _ => {
                let press = session.image_viewer.thumbnail_press.as_mut()?;
                press.cancelled = true;
                Some(Effect::Nothing)
            }
        }
    }
}

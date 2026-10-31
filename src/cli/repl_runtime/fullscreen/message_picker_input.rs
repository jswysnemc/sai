//! 打开的消息面板优先消费事件，避免列表操作进入输入框或下方正文。

use super::input::Effect;
use super::message_picker_view::PickerLayout;
use crate::cli::repl_runtime::ReplRuntime;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

impl ReplRuntime {
    /// 【消息导航】【事件隔离】面板打开时导航、确认和取消均由列表处理。
    /// 参数: event 为终端事件；返回: 面板处理效果，未打开时为空。
    pub(super) fn message_picker_event(&mut self, event: &Event) -> Option<Effect> {
        let session = self.fullscreen.as_mut()?;
        if !session.state.message_picker.active {
            return None;
        }
        let state = &mut session.state;
        let layout = state.layout?;
        let count = state.document.anchors.len();
        let geometry = PickerLayout::compute(&layout, count);
        let picker = &mut state.message_picker;
        picker.sync(count, geometry.visible);
        let mut jump = None;
        let mut changed = false;
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Release => return Some(Effect::Nothing),
            Event::Key(key) => {
                // 1. 【消息导航】【全局快捷键】中断和退出全屏保留原行为，其余编辑按键不修改草稿
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'o'))
                {
                    picker.close();
                    return Some(Effect::Pass);
                }
                match key.code {
                    KeyCode::Esc => {
                        picker.close();
                        changed = true;
                    }
                    KeyCode::Enter => jump = Some(picker.selected),
                    KeyCode::Up => changed = picker.move_by(-1, count, geometry.visible),
                    KeyCode::Down => changed = picker.move_by(1, count, geometry.visible),
                    KeyCode::PageUp => {
                        changed =
                            picker.move_by(-(geometry.visible as isize), count, geometry.visible)
                    }
                    KeyCode::PageDown => {
                        changed = picker.move_by(geometry.visible as isize, count, geometry.visible)
                    }
                    KeyCode::Home => changed = picker.select(0, count, geometry.visible),
                    KeyCode::End => {
                        changed = picker.select(count.saturating_sub(1), count, geometry.visible)
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                let item = geometry.item_at(mouse.column, mouse.row, picker.start);
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(index) = item {
                            jump = Some(index);
                        } else if !geometry.contains(mouse.column, mouse.row) {
                            picker.close();
                            changed = true;
                        }
                    }
                    MouseEventKind::Moved => {
                        if let Some(index) = item {
                            changed = picker.select(index, count, geometry.visible);
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        changed = picker.move_by(-1, count, geometry.visible)
                    }
                    MouseEventKind::ScrollDown => {
                        changed = picker.move_by(1, count, geometry.visible)
                    }
                    _ => {}
                }
            }
            Event::Resize(..) => return Some(Effect::Pass),
            Event::Paste(_) => {}
            _ => return Some(Effect::Pass),
        }
        // 2. 【消息导航】【确认跳转】关闭后定位用户消息，面板自身不改动展开键或正文
        if let Some(index) = jump {
            picker.close();
            state.jump_to_anchor(index, usize::from(layout.body_height));
            state.header_hover = false;
            state.pointer_row = None;
            state.pointer_col = None;
            changed = true;
        }
        Some(if changed {
            Effect::Repaint
        } else {
            Effect::Nothing
        })
    }
}

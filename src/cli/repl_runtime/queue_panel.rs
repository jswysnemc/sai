//! 队列管理焦点、稳定选择和删除确认，不负责绘制或消费消息。

use super::QueuedSubmission;
use crossterm::event::{KeyCode, KeyModifiers};
use std::collections::VecDeque;

#[derive(Default)]
pub(super) struct QueuePanelState {
    pub(super) active: bool,
    pub(super) selected: usize,
    pub(super) selected_id: Option<String>,
    pub(super) pending_delete: bool,
    pub(super) scroll: usize,
    pub(super) deleted: VecDeque<(usize, QueuedSubmission)>,
    pub(super) notice: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum QueuePanelAction {
    Ignored,
    Consumed,
    Exit,
    Delete,
    Edit,
    SendNow,
    ToggleInsert,
    MoveUp,
    MoveDown,
    Top,
    UndoDelete,
}

impl QueuePanelState {
    /// 【用户队列】【焦点查询】参数: 无；返回: 是否处于管理焦点。
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// 【用户队列】【选择查询】参数: 无；返回: 当前可见消息下标。
    pub(super) fn selected(&self) -> usize {
        self.selected
    }

    /// 【用户队列】【进入管理】默认选择最靠近输入框的队尾，空队列有删除历史时允许恢复。
    /// 参数: len 为队列长度；返回: 是否进入管理。
    pub(super) fn activate(&mut self, len: usize) -> bool {
        if len == 0 && self.deleted.is_empty() {
            return false;
        }
        self.active = true;
        self.selected = len.saturating_sub(1);
        self.selected_id = None;
        self.pending_delete = false;
        self.notice = None;
        true
    }

    /// 【用户队列】【返回输入】清除操作焦点，不清除可撤销的删除历史。
    /// 参数: 无；返回: 无。
    pub(super) fn deactivate(&mut self) {
        self.active = false;
        self.selected = 0;
        self.selected_id = None;
        self.pending_delete = false;
        self.scroll = 0;
        self.notice = None;
    }

    /// 【用户队列】【数量同步】长度收缩后保留合法选择，删除历史存在时不强制关闭恢复入口。
    /// 参数: len 为最新长度；返回: 无。
    pub(super) fn clamp(&mut self, len: usize) {
        if len == 0 && self.deleted.is_empty() {
            self.deactivate();
            return;
        }
        self.selected = self.selected.min(len.saturating_sub(1));
    }

    /// 【用户队列】【稳定选择】后台消费后跟随原消息，原消息消失时取消删除确认。
    /// 参数: ids 为最新消息标识；返回: 无。
    pub(super) fn sync_ids(&mut self, ids: &[String]) {
        if let Some(id) = &self.selected_id {
            if let Some(index) = ids.iter().position(|candidate| candidate == id) {
                self.selected = index;
            } else {
                self.pending_delete = false;
                self.selected_id = None;
            }
        }
        self.clamp(ids.len());
        self.selected_id = ids.get(self.selected).cloned();
        let visible = super::queue_panel_view::page_size();
        self.scroll = self.scroll.min(ids.len().saturating_sub(visible));
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
    }

    /// 【用户队列】【定位消息】键盘和鼠标共用选择入口，切换消息后取消旧删除确认。
    /// 参数: index 为新下标；返回: 无。
    pub(super) fn select(&mut self, index: usize) {
        self.selected = index;
        self.selected_id = None;
        self.pending_delete = false;
        self.notice = None;
    }

    /// 【用户队列】【按键动作】焦点态只派发管理动作，删除确认必须连续两次针对同一条消息。
    /// 参数: code/modifiers 为按键，len 为队列数量；返回: 管理动作。
    pub(super) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        len: usize,
    ) -> QueuePanelAction {
        if !self.active {
            return QueuePanelAction::Ignored;
        }
        if code == KeyCode::Esc {
            if self.pending_delete {
                self.pending_delete = false;
                return QueuePanelAction::Consumed;
            }
            self.deactivate();
            return QueuePanelAction::Exit;
        }
        if matches!(code, KeyCode::Char('u' | 'U')) && !modifiers.contains(KeyModifiers::CONTROL)
            || code == KeyCode::Char('z') && modifiers.contains(KeyModifiers::CONTROL)
        {
            self.pending_delete = false;
            return QueuePanelAction::UndoDelete;
        }
        self.clamp(len);
        if len == 0 {
            return QueuePanelAction::Consumed;
        }
        if matches!(code, KeyCode::Char('d' | 'D') | KeyCode::Delete) {
            if self.pending_delete {
                self.pending_delete = false;
                return QueuePanelAction::Delete;
            }
            self.pending_delete = true;
            return QueuePanelAction::Consumed;
        }
        self.pending_delete = false;
        self.notice = None;
        let shift = modifiers.contains(KeyModifiers::SHIFT);
        match code {
            KeyCode::Up if shift => QueuePanelAction::MoveUp,
            KeyCode::Down if shift => QueuePanelAction::MoveDown,
            KeyCode::Char('k') if modifiers.is_empty() => QueuePanelAction::MoveUp,
            KeyCode::Char('j') if modifiers.is_empty() => QueuePanelAction::MoveDown,
            KeyCode::Char('t' | 'T') if !modifiers.contains(KeyModifiers::CONTROL) => {
                QueuePanelAction::Top
            }
            KeyCode::PageUp => {
                self.select(
                    self.selected
                        .saturating_sub(super::queue_panel_view::page_size()),
                );
                QueuePanelAction::Consumed
            }
            KeyCode::PageDown => {
                self.select((self.selected + super::queue_panel_view::page_size()).min(len - 1));
                QueuePanelAction::Consumed
            }
            KeyCode::Home => {
                self.select(0);
                QueuePanelAction::Consumed
            }
            KeyCode::End => {
                self.select(len - 1);
                QueuePanelAction::Consumed
            }
            KeyCode::Up => {
                self.select(self.selected.saturating_sub(1));
                QueuePanelAction::Consumed
            }
            KeyCode::Down if self.selected + 1 >= len => {
                self.deactivate();
                QueuePanelAction::Exit
            }
            KeyCode::Down => {
                self.select(self.selected + 1);
                QueuePanelAction::Consumed
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::CONTROL) => {
                QueuePanelAction::SendNow
            }
            KeyCode::Enter if modifiers.is_empty() => QueuePanelAction::Edit,
            KeyCode::Char('s' | 'S') if !modifiers.contains(KeyModifiers::CONTROL) => {
                QueuePanelAction::SendNow
            }
            KeyCode::Char('e' | 'E') if !modifiers.contains(KeyModifiers::CONTROL) => {
                QueuePanelAction::Edit
            }
            KeyCode::Tab if modifiers.is_empty() => QueuePanelAction::ToggleInsert,
            _ => QueuePanelAction::Consumed,
        }
    }

    /// 【用户队列】【面板组合】只返回限高视图行，完整内容保持在消息队列中。
    /// 参数: queued 为快照，streaming 为运行状态；返回: ANSI 面板行。
    pub(super) fn panel_lines(&self, queued: &[QueuedSubmission], streaming: bool) -> Vec<String> {
        super::queue_panel_view::render(self, queued, streaming)
            .into_iter()
            .map(|row| row.text)
            .collect()
    }
}

/// 【用户队列】【管理快捷键】参数: code/modifiers 为按键；返回: 是否为 Ctrl+↑ 或 Alt+M。
pub(in crate::cli) fn is_enter_queue_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    code == KeyCode::Up && modifiers.contains(KeyModifiers::CONTROL)
        || matches!(code, KeyCode::Char('m' | 'M')) && modifiers.contains(KeyModifiers::ALT)
}

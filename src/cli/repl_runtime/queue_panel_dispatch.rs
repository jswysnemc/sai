//! 空闲和流式输入的队列管理派发。

use super::queue_panel::{is_enter_queue_key, QueuePanelAction};
use super::{QueueInsertAt, QueuedSubmission};
use crossterm::event::{KeyCode, KeyModifiers};

/// 空闲输入阶段的队列面板处理结果。
#[derive(Debug)]
pub(in crate::cli) enum QueuePanelIdleResult {
    /// 按键与面板无关
    Ignored,
    /// 已处理，调用方重绘输入框
    Consumed,
    /// 取回当前项到输入框编辑
    Edit(QueuedSubmission),
    /// 立即作为用户提交发出
    SendNow(QueuedSubmission),
}

/// 内部派发结果：流式阶段消化 Edit/SendNow，空闲阶段交回调用方。
enum QueuePanelOutcome {
    Ignored,
    Consumed,
    Edit(QueuedSubmission),
    SendNow(QueuedSubmission),
}

impl super::ReplRuntime {
    /// 【用户队列】【草稿时机】指定本次草稿提交策略，取回编辑默认沿用原策略。
    /// 参数: timing 为消费时机；返回: 无。
    pub(in crate::cli) fn set_queue_draft_timing(&mut self, timing: QueueInsertAt) {
        self.queue_draft_timing = timing;
    }
    /// 返回用户消息队列面板是否处于焦点态。
    pub(in crate::cli) fn queue_panel_active(&self) -> bool {
        self.queue_panel.is_active()
    }

    /// 处理流式阶段的队列面板按键。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `modifiers`: 修饰键
    ///
    /// 返回:
    /// - 按键被面板消费时为 true
    pub(in crate::cli) fn handle_queue_panel_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> anyhow::Result<bool> {
        let draft_empty = self.stream_draft.text.trim().is_empty();
        match self.queue_panel_dispatch(code, modifiers, true, draft_empty)? {
            QueuePanelOutcome::Ignored => Ok(false),
            _ => {
                self.redraw_stream_composer()?;
                Ok(true)
            }
        }
    }

    /// 处理空闲输入阶段的队列面板按键。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `modifiers`: 修饰键
    /// - `draft_empty`: 输入框是否为空（取回编辑时不覆盖非空草稿）
    ///
    /// 返回:
    /// - 空闲阶段面板结果
    pub(in crate::cli) fn handle_queue_panel_idle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        draft_empty: bool,
    ) -> anyhow::Result<QueuePanelIdleResult> {
        Ok(
            match self.queue_panel_dispatch(code, modifiers, false, draft_empty)? {
                QueuePanelOutcome::Ignored => QueuePanelIdleResult::Ignored,
                QueuePanelOutcome::Consumed => QueuePanelIdleResult::Consumed,
                QueuePanelOutcome::Edit(item) => QueuePanelIdleResult::Edit(item),
                QueuePanelOutcome::SendNow(item) => QueuePanelIdleResult::SendNow(item),
            },
        )
    }

    /// 队列面板按键核心处理。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `modifiers`: 修饰键
    /// - `streaming`: 是否处于模型流式阶段
    /// - `draft_empty`: 当前输入草稿是否为空
    ///
    /// 返回:
    /// - 内部派发结果
    fn queue_panel_dispatch(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        streaming: bool,
        draft_empty: bool,
    ) -> anyhow::Result<QueuePanelOutcome> {
        if !self.queue_panel.is_active() {
            if !is_enter_queue_key(code, modifiers) {
                return Ok(QueuePanelOutcome::Ignored);
            }
            if !self.queue_panel.activate(self.queue_len()) {
                return Ok(QueuePanelOutcome::Ignored);
            }
            self.agent_panel.deactivate();
            self.clamp_queue_panel();
            return Ok(QueuePanelOutcome::Consumed);
        }
        self.clamp_queue_panel();
        let queue_len = self.queue_len();
        let action = self.queue_panel.handle_key(code, modifiers, queue_len);
        let result = match action {
            QueuePanelAction::Ignored => Ok(QueuePanelOutcome::Ignored),
            QueuePanelAction::Consumed | QueuePanelAction::Exit => Ok(QueuePanelOutcome::Consumed),
            QueuePanelAction::ToggleInsert => {
                self.toggle_queued_insert_at();
                Ok(QueuePanelOutcome::Consumed)
            }
            QueuePanelAction::Delete => {
                self.delete_queued_at();
                Ok(QueuePanelOutcome::Consumed)
            }
            QueuePanelAction::MoveUp | QueuePanelAction::MoveDown | QueuePanelAction::Top => {
                self.move_queued(
                    if action == QueuePanelAction::MoveDown {
                        1
                    } else {
                        -1
                    },
                    action == QueuePanelAction::Top,
                );
                Ok(QueuePanelOutcome::Consumed)
            }
            QueuePanelAction::UndoDelete => {
                self.restore_deleted_queued();
                Ok(QueuePanelOutcome::Consumed)
            }
            QueuePanelAction::Edit => {
                if !draft_empty {
                    self.queue_panel.notice = Some(
                        crate::i18n::text(
                            "clear the input box before retrieving a queued message",
                            "请先清空输入框再取回排队消息",
                        )
                        .to_string(),
                    );
                    return Ok(QueuePanelOutcome::Consumed);
                }
                let Some(item) = self.take_queued_at() else {
                    return Ok(QueuePanelOutcome::Consumed);
                };
                self.queue_panel.deactivate();
                self.queue_draft_timing = item.insert_at;
                if streaming {
                    self.apply_queued_to_stream_draft(item);
                    Ok(QueuePanelOutcome::Consumed)
                } else {
                    Ok(QueuePanelOutcome::Edit(item))
                }
            }
            QueuePanelAction::SendNow => {
                let id = self.queue_panel.selected_id.clone();
                let item = self.take_queued_at();
                let Some(mut item) = item else {
                    self.clamp_queue_panel();
                    return Ok(QueuePanelOutcome::Consumed);
                };
                // 1. 【终端队列】【显式发送】用户选择立即发送时解除失败暂停
                self.resume_submission_queue();
                if streaming {
                    // 先取出再入队首，中间不能 clamp：只剩一项时会误判为空而退出
                    item.insert_at = QueueInsertAt::Request;
                    self.lock_queue().push_front(item);
                    self.queue_panel.active = true;
                    self.queue_panel.selected_id = id;
                    self.clamp_queue_panel();
                    self.queue_panel.selected = 0;
                    Ok(QueuePanelOutcome::Consumed)
                } else {
                    self.clamp_queue_panel();
                    self.queue_panel.deactivate();
                    Ok(QueuePanelOutcome::SendNow(item))
                }
            }
        };
        self.clamp_queue_panel();
        result
    }
}

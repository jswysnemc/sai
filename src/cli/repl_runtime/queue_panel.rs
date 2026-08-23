use crate::cli::repl_runtime::QueuedSubmission;
use crate::i18n::text as t;
use crossterm::event::{KeyCode, KeyModifiers};

/// 用户消息队列管理面板的交互状态。
#[derive(Default)]
pub(super) struct QueuePanelState {
    /// 是否处于管理焦点态（Ctrl+↑ 进入，↓ 离开末项回到输入框）
    active: bool,
    /// 当前高亮下标，对应消息队列的先进先出顺序
    selected: usize,
}

/// 面板按键处理结果。
#[derive(Debug, Eq, PartialEq)]
pub(super) enum QueuePanelAction {
    /// 按键与面板无关
    Ignored,
    /// 仅需重绘
    Consumed,
    /// 退出管理，焦点回到输入框
    Exit,
    /// 删除当前项
    Delete,
    /// 取回当前项到输入框编辑
    Edit,
    /// 立即发送当前项（流式阶段提到队首，空闲阶段立刻提交）
    SendNow,
}

impl QueuePanelState {
    /// 返回面板是否处于焦点态。
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// 返回当前高亮下标。
    pub(super) fn selected(&self) -> usize {
        self.selected
    }

    /// 尝试进入管理焦点态。
    ///
    /// 默认高亮最靠近输入框的队尾项：Ctrl+↑ 从输入框「往上一格」即落到它。
    ///
    /// 参数:
    /// - `len`: 当前用户队列长度
    ///
    /// 返回:
    /// - 队列非空并已进入时为 true
    pub(super) fn activate(&mut self, len: usize) -> bool {
        if len == 0 {
            return false;
        }
        self.active = true;
        self.selected = len - 1;
        true
    }

    /// 退出焦点态。
    pub(super) fn deactivate(&mut self) {
        self.active = false;
        self.selected = 0;
    }

    /// 队列长度变化后夹紧高亮，空队列时退出。
    ///
    /// 参数:
    /// - `len`: 最新队列长度
    ///
    /// 返回:
    /// - 无
    pub(super) fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.deactivate();
            return;
        }
        if self.selected >= len {
            self.selected = len - 1;
        }
    }

    /// 处理焦点态下的按键。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `modifiers`: 修饰键
    /// - `len`: 当前用户队列长度
    ///
    /// 返回:
    /// - 面板动作
    pub(super) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        len: usize,
    ) -> QueuePanelAction {
        if !self.active {
            return QueuePanelAction::Ignored;
        }
        if len == 0 {
            self.deactivate();
            return QueuePanelAction::Exit;
        }
        self.clamp(len);
        match code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                QueuePanelAction::Consumed
            }
            KeyCode::Down => {
                if self.selected + 1 >= len {
                    self.deactivate();
                    QueuePanelAction::Exit
                } else {
                    self.selected += 1;
                    QueuePanelAction::Consumed
                }
            }
            KeyCode::Esc => {
                self.deactivate();
                QueuePanelAction::Exit
            }
            KeyCode::Enter if modifiers.contains(KeyModifiers::CONTROL) => {
                QueuePanelAction::SendNow
            }
            KeyCode::Enter if modifiers.is_empty() => QueuePanelAction::Edit,
            KeyCode::Char('s') | KeyCode::Char('S')
                if !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                QueuePanelAction::SendNow
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Backspace | KeyCode::Delete => {
                QueuePanelAction::Delete
            }
            KeyCode::Char('e') | KeyCode::Char('E')
                if !modifiers.contains(KeyModifiers::CONTROL) =>
            {
                QueuePanelAction::Edit
            }
            // 管理态吞掉其它输入，避免落到输入框；Ctrl+C 由外层先处理
            _ => QueuePanelAction::Consumed,
        }
    }

    /// 渲染队列区行。
    ///
    /// 非焦点：最多三行预览并提示 Ctrl+↑。焦点：列出全部条目并高亮当前项。
    ///
    /// 参数:
    /// - `queued`: 用户消息队列
    ///
    /// 返回:
    /// - 未截断的 ANSI 行；队列为空时为空
    pub(super) fn panel_lines(&self, queued: &[QueuedSubmission]) -> Vec<String> {
        if queued.is_empty() {
            return Vec::new();
        }
        if self.active {
            self.active_lines(queued)
        } else {
            self.idle_lines(queued)
        }
    }

    /// 非焦点态的摘要行。
    fn idle_lines(&self, queued: &[QueuedSubmission]) -> Vec<String> {
        // 只保留一行：预览条目会抬高输入框
        let preview = queued
            .last()
            .map(|submission| preview_text(&submission.text))
            .unwrap_or_default();
        vec![format!(
            "\x1b[2m• {} ({})  {}  \x1b[3m{}\x1b[0m",
            t("queued for next turn", "已排队待下一轮"),
            queued.len(),
            t("Ctrl+↑ manage", "Ctrl+↑ 管理"),
            preview
        )]
    }

    /// 焦点态列出全部条目。
    fn active_lines(&self, queued: &[QueuedSubmission]) -> Vec<String> {
        let mut lines = vec![format!(
            "\x1b[2m• {} · ↑↓ {} · Enter {} · d {} · s {} · ↓ {}\x1b[0m",
            t("queued messages", "排队消息"),
            t("select", "选择"),
            t("edit", "取回编辑"),
            t("delete", "删除"),
            t("send now", "立即发送"),
            t("input", "回输入框")
        )];
        for (index, submission) in queued.iter().enumerate() {
            let preview = preview_text(&submission.text);
            if index == self.selected {
                lines.push(format!("\x1b[1m\x1b[36m  ❯ {preview}\x1b[0m"));
            } else {
                lines.push(format!("\x1b[2m    {preview}\x1b[0m"));
            }
        }
        lines
    }
}

/// 判断按键是否为进入队列管理的快捷键。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键
///
/// 返回:
/// - Ctrl+↑ 时为 true
pub(super) fn is_enter_queue_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    code == KeyCode::Up && modifiers.contains(KeyModifiers::CONTROL)
}

/// 把排队正文压成单行预览。
fn preview_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

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
            if !self.queue_panel.activate(self.submission_queue.len()) {
                return Ok(QueuePanelOutcome::Ignored);
            }
            self.agent_panel.deactivate();
            return Ok(QueuePanelOutcome::Consumed);
        }
        match self
            .queue_panel
            .handle_key(code, modifiers, self.submission_queue.len())
        {
            QueuePanelAction::Ignored => Ok(QueuePanelOutcome::Ignored),
            QueuePanelAction::Consumed | QueuePanelAction::Exit => Ok(QueuePanelOutcome::Consumed),
            QueuePanelAction::Delete => {
                self.delete_queued_at(self.queue_panel.selected());
                Ok(QueuePanelOutcome::Consumed)
            }
            QueuePanelAction::Edit => {
                if !draft_empty {
                    self.record_meta(
                        crate::i18n::text(
                            "clear the input box before retrieving a queued message",
                            "请先清空输入框再取回排队消息",
                        )
                        .to_string(),
                    )?;
                    return Ok(QueuePanelOutcome::Consumed);
                }
                let Some(item) = self.take_queued_at(self.queue_panel.selected()) else {
                    return Ok(QueuePanelOutcome::Consumed);
                };
                self.queue_panel.deactivate();
                if streaming {
                    self.apply_queued_to_stream_draft(item);
                    Ok(QueuePanelOutcome::Consumed)
                } else {
                    Ok(QueuePanelOutcome::Edit(item))
                }
            }
            QueuePanelAction::SendNow => {
                let index = self.queue_panel.selected();
                if index >= self.submission_queue.len() {
                    return Ok(QueuePanelOutcome::Consumed);
                }
                let Some(item) = self.submission_queue.remove(index) else {
                    return Ok(QueuePanelOutcome::Consumed);
                };
                if streaming {
                    // 先取出再入队首，中间不能 clamp：只剩一项时会误判为空而退出
                    self.submission_queue.push_front(item);
                    self.queue_panel.clamp(self.submission_queue.len());
                    self.queue_panel.selected = 0;
                    Ok(QueuePanelOutcome::Consumed)
                } else {
                    self.queue_panel.clamp(self.submission_queue.len());
                    self.queue_panel.deactivate();
                    Ok(QueuePanelOutcome::SendNow(item))
                }
            }
        }
    }

    /// 删除指定下标的用户排队项并夹紧高亮。
    fn delete_queued_at(&mut self, index: usize) {
        if index < self.submission_queue.len() {
            self.submission_queue.remove(index);
        }
        self.queue_panel.clamp(self.submission_queue.len());
    }

    /// 取出指定下标的用户排队项并夹紧高亮。
    fn take_queued_at(&mut self, index: usize) -> Option<QueuedSubmission> {
        if index >= self.submission_queue.len() {
            self.queue_panel.clamp(self.submission_queue.len());
            return None;
        }
        let item = self.submission_queue.remove(index);
        self.queue_panel.clamp(self.submission_queue.len());
        item
    }

    /// 把排队项交还流式草稿。
    fn apply_queued_to_stream_draft(&mut self, item: QueuedSubmission) {
        self.stream_draft.clipboard = item.clipboard;
        self.stream_draft.mode = Some(item.mode);
        self.stream_draft.text = item.text;
        self.stream_draft.cursor = self.stream_draft.text.chars().count();
        self.stream_draft.slash_selection = 0;
        self.stream_draft.is_pasted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::cli::repl_clipboard::ReplClipboardState;

    fn item(text: &str) -> QueuedSubmission {
        QueuedSubmission {
            mode: AgentMode::Yolo,
            text: text.to_string(),
            clipboard: ReplClipboardState::default(),
        }
    }

    #[test]
    fn activate_selects_the_item_nearest_the_input() {
        let mut panel = QueuePanelState::default();
        assert!(!panel.activate(0));
        assert!(panel.activate(3));
        assert_eq!(panel.selected(), 2);
        assert!(panel.is_active());
    }

    #[test]
    fn down_on_last_item_returns_to_input() {
        let mut panel = QueuePanelState::default();
        panel.activate(2);
        assert_eq!(panel.selected(), 1);
        assert_eq!(
            panel.handle_key(KeyCode::Down, KeyModifiers::NONE, 2),
            QueuePanelAction::Exit
        );
        assert!(!panel.is_active());
    }

    #[test]
    fn down_on_a_higher_item_moves_toward_the_input() {
        let mut panel = QueuePanelState::default();
        panel.activate(3);
        assert_eq!(
            panel.handle_key(KeyCode::Up, KeyModifiers::NONE, 3),
            QueuePanelAction::Consumed
        );
        assert_eq!(panel.selected(), 1);
        assert_eq!(
            panel.handle_key(KeyCode::Down, KeyModifiers::NONE, 3),
            QueuePanelAction::Consumed
        );
        assert_eq!(panel.selected(), 2);
        assert!(panel.is_active());
    }

    #[test]
    fn up_stops_at_the_first_item() {
        let mut panel = QueuePanelState::default();
        panel.activate(2);
        panel.selected = 0;
        assert_eq!(
            panel.handle_key(KeyCode::Up, KeyModifiers::NONE, 2),
            QueuePanelAction::Consumed
        );
        assert_eq!(panel.selected(), 0);
    }

    #[test]
    fn enter_edits_and_s_sends() {
        let mut panel = QueuePanelState::default();
        panel.activate(1);
        assert_eq!(
            panel.handle_key(KeyCode::Enter, KeyModifiers::NONE, 1),
            QueuePanelAction::Edit
        );
        assert_eq!(
            panel.handle_key(KeyCode::Char('s'), KeyModifiers::NONE, 1),
            QueuePanelAction::SendNow
        );
        assert_eq!(
            panel.handle_key(KeyCode::Char('d'), KeyModifiers::NONE, 1),
            QueuePanelAction::Delete
        );
    }

    #[test]
    fn idle_preview_mentions_ctrl_up() {
        let panel = QueuePanelState::default();
        let lines = panel.panel_lines(&[item("one"), item("two"), item("three"), item("four")]);
        let joined = lines.join("\n");
        assert_eq!(lines.len(), 1);
        assert!(joined.contains("Ctrl+↑") || joined.contains("Ctrl+Up"));
        assert!(joined.contains("(4)"));
        assert!(joined.contains("four"));
        assert!(!joined.contains('❯'));
    }

    #[test]
    fn active_list_highlights_selection() {
        let mut panel = QueuePanelState::default();
        panel.activate(2);
        let lines = panel.panel_lines(&[item("first"), item("second")]);
        let joined = lines.join("\n");
        assert!(joined.contains('❯'));
        assert!(joined.contains("second"));
        assert!(joined.contains("first"));
    }
}

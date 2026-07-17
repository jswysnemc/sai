use crate::permission::PermissionDecision;
use crate::render::PermissionChoice;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

/// 权限审计交互状态（CLI 与 TUI 共用）。
#[derive(Debug, Clone)]
pub(crate) struct PermissionInteractionState {
    selected: PermissionChoice,
    reply_draft: Option<String>,
}

/// 单次事件处理后的交互结果。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum PermissionTransition {
    /// 仅刷新 UI，继续等待输入
    Continue,
    /// 用户已提交最终决定
    Submit(PermissionDecision),
}

impl PermissionInteractionState {
    /// 创建默认高亮「允许一次」的交互状态。
    ///
    /// 返回:
    /// - 新的交互状态
    pub(crate) fn new() -> Self {
        Self {
            selected: PermissionChoice::Allow,
            reply_draft: None,
        }
    }

    /// 返回当前高亮选项。
    ///
    /// 返回:
    /// - 权限选择项
    pub(crate) fn selected(&self) -> PermissionChoice {
        self.selected
    }

    /// 返回当前拒绝回复草稿。
    ///
    /// 返回:
    /// - 草稿文本；未进入回复编辑时为 `None`
    pub(crate) fn reply_draft(&self) -> Option<&str> {
        self.reply_draft.as_deref()
    }

    /// 处理终端事件并更新交互状态。
    ///
    /// 参数:
    /// - `event`: Crossterm 终端事件
    ///
    /// 返回:
    /// - 继续等待或提交最终决定
    pub(crate) fn handle_event(&mut self, event: Event) -> PermissionTransition {
        if let Event::Paste(text) = event {
            if let Some(draft) = self.reply_draft.as_mut() {
                draft.push_str(&text);
            }
            return PermissionTransition::Continue;
        }
        let Event::Key(key) = event else {
            return PermissionTransition::Continue;
        };
        if key.kind == KeyEventKind::Release {
            return PermissionTransition::Continue;
        }
        if let Some(draft) = self.reply_draft.as_mut() {
            return match key.code {
                KeyCode::Enter => PermissionTransition::Submit(PermissionDecision::Deny {
                    reply: (!draft.trim().is_empty()).then(|| draft.trim().to_string()),
                }),
                KeyCode::Esc => {
                    self.reply_draft = None;
                    PermissionTransition::Continue
                }
                KeyCode::Backspace => {
                    draft.pop();
                    PermissionTransition::Continue
                }
                KeyCode::Char(value) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    draft.push(value);
                    PermissionTransition::Continue
                }
                _ => PermissionTransition::Continue,
            };
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.prev();
                PermissionTransition::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = self.selected.next();
                PermissionTransition::Continue
            }
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('1') => {
                PermissionTransition::Submit(PermissionDecision::Allow)
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('2') | KeyCode::Esc => {
                PermissionTransition::Submit(PermissionDecision::Deny { reply: None })
            }
            KeyCode::Char('3') => {
                self.selected = PermissionChoice::DenyWithReply;
                self.reply_draft = Some(String::new());
                PermissionTransition::Continue
            }
            KeyCode::Enter => match self.selected {
                PermissionChoice::Allow => PermissionTransition::Submit(PermissionDecision::Allow),
                PermissionChoice::Deny => {
                    PermissionTransition::Submit(PermissionDecision::Deny { reply: None })
                }
                PermissionChoice::DenyWithReply => {
                    self.reply_draft = Some(String::new());
                    PermissionTransition::Continue
                }
            },
            _ => PermissionTransition::Continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyEventState};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn down_and_enter_submit_denial() {
        let mut state = PermissionInteractionState::new();
        assert_eq!(
            state.handle_event(key(KeyCode::Down)),
            PermissionTransition::Continue
        );
        assert_eq!(state.selected(), PermissionChoice::Deny);
        assert_eq!(
            state.handle_event(key(KeyCode::Enter)),
            PermissionTransition::Submit(PermissionDecision::Deny { reply: None })
        );
    }

    #[test]
    fn denial_reply_supports_editing() {
        let mut state = PermissionInteractionState::new();
        state.handle_event(key(KeyCode::Char('3')));
        state.handle_event(key(KeyCode::Char('a')));
        state.handle_event(key(KeyCode::Char('b')));
        state.handle_event(key(KeyCode::Backspace));
        assert_eq!(
            state.handle_event(key(KeyCode::Enter)),
            PermissionTransition::Submit(PermissionDecision::Deny {
                reply: Some("a".to_string())
            })
        );
    }
}

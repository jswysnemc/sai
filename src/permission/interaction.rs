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
    /// 【权限交互】【初始化】创建默认高亮「允许一次」的交互状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 新的交互状态
    pub(crate) fn new() -> Self {
        Self {
            selected: PermissionChoice::Allow,
            reply_draft: None,
        }
    }

    /// 【权限交互】【当前选择】返回当前高亮选项。
    ///
    /// 参数:
    /// - `self`: 交互状态
    ///
    /// 返回:
    /// - 权限选择项
    pub(crate) fn selected(&self) -> PermissionChoice {
        self.selected
    }

    /// 【权限交互】【回复草稿】返回当前拒绝回复草稿。
    ///
    /// 参数:
    /// - `self`: 交互状态
    ///
    /// 返回:
    /// - 草稿文本；未进入回复编辑时为空
    pub(crate) fn reply_draft(&self) -> Option<&str> {
        self.reply_draft.as_deref()
    }

    /// 【权限交互】【事件处理】处理键盘与括号粘贴事件并更新状态机。
    ///
    /// 参数:
    /// - `event`: Crossterm 终端事件
    ///
    /// 返回:
    /// - 继续等待或提交最终决定
    pub(crate) fn handle_event(&mut self, event: Event) -> PermissionTransition {
        if let Event::Paste(text) = event {
            // 1. 【权限交互】【粘贴净化】过滤回车换行与控制序列，防止注入转义码或破坏单行草稿布局
            if let Some(draft) = self.reply_draft.as_mut() {
                let cleaned = sanitize_paste_reply(&text);
                draft.push_str(&cleaned);
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
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    draft.clear();
                    PermissionTransition::Continue
                }
                KeyCode::Backspace => {
                    draft.pop();
                    PermissionTransition::Continue
                }
                KeyCode::Char(value)
                    if !key.modifiers.contains(KeyModifiers::CONTROL) && !value.is_control() =>
                {
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
                PermissionTransition::Submit(PermissionDecision::allow_once())
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
                PermissionChoice::Allow => {
                    PermissionTransition::Submit(PermissionDecision::allow_once())
                }
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

/// 【权限交互】【净化粘贴】剥离 ANSI/OSC 控制序列并剔除回车、换行与其他控制字符。
///
/// 参数:
/// - `text`: 粘贴原始文本
///
/// 返回:
/// - 安全的单行回复内容
fn sanitize_paste_reply(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\x07' {
                            break;
                        }
                        if next == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some('('..='/') => {
                    chars.next();
                    chars.next();
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
            continue;
        }
        if ch == '\t' {
            output.push(' ');
        } else if !ch.is_control() {
            output.push(ch);
        }
    }
    output
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

    /// 【权限交互测试】【粘贴安全】验证回复草稿粘贴时自动剥离换行与控制序列且不误提交。
    #[test]
    fn paste_in_reply_draft_strips_newlines_and_control_sequences() {
        let mut state = PermissionInteractionState::new();
        state.handle_event(key(KeyCode::Char('3')));
        let outcome = state.handle_event(Event::Paste(
            "请改用只读命令\r\n\x1b[31m不要修改配置\x07\t谢谢".into(),
        ));
        assert_eq!(outcome, PermissionTransition::Continue);
        assert_eq!(
            state.reply_draft(),
            Some("请改用只读命令不要修改配置 谢谢")
        );
    }
}

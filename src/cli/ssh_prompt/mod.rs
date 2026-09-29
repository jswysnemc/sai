//! SSH 交互征询的终端前端：TUI 在输入框上方挂卡片，CLI 流式模式逐行提示。
//!
//! 两个前端共用 [`PromptInput`] 状态机：按键只改状态，不直接写终端，
//! 秘密文本只保存在状态机内，提交后即清零。

mod card;
mod cli;
mod tui;

pub(super) use card::SshPromptView;
pub(super) use cli::prompt_ssh_secret_request_cli;
pub(super) use tui::prompt_ssh_secret_request_tui;

use crate::ssh::{InteractiveKind, SecretRequest, SecretResponse};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

/// 单个秘密最多接受的字符数，防止误粘贴大段文本。
const MAX_SECRET_CHARS: usize = 4096;

/// 一次按键处理后的结果。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PromptStep {
    /// 继续等待输入，界面需要刷新
    Continue,
    /// 已得到最终应答
    Done(SecretResponse),
}

/// 秘密输入与是/否确认的共享状态机。
pub(super) struct PromptInput {
    kind: InteractiveKind,
    secret: String,
}

impl PromptInput {
    /// 按征询类型创建状态机。
    ///
    /// 参数:
    /// - `request`: 交互征询（不含秘密）
    ///
    /// 返回:
    /// - 初始状态
    pub(super) fn new(request: &SecretRequest) -> Self {
        Self {
            kind: request.kind,
            secret: String::new(),
        }
    }

    /// 当前是否为秘密输入（否则为是/否确认）。
    ///
    /// 返回:
    /// - 口令或密码类征询为 `true`
    pub(super) fn is_secret(&self) -> bool {
        is_secret_kind(self.kind)
    }

    /// 已输入字符数，仅用于界面提示“已输入”，不展示长度。
    ///
    /// 返回:
    /// - 是否已有输入
    pub(super) fn has_input(&self) -> bool {
        !self.secret.is_empty()
    }

    /// 处理一个终端事件。
    ///
    /// 参数:
    /// - `event`: 终端事件
    ///
    /// 返回:
    /// - 继续等待或最终应答
    pub(super) fn handle(&mut self, event: Event) -> PromptStep {
        if super::permission_prompt::is_interrupt(&event) {
            return self.finish(SecretResponse::Cancelled);
        }
        match event {
            Event::Paste(text) if self.is_secret() => {
                self.push_text(&text);
                PromptStep::Continue
            }
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if self.is_secret() {
                    self.handle_secret_key(key.code, key.modifiers)
                } else {
                    Self::handle_confirm_key(key.code)
                }
            }
            _ => PromptStep::Continue,
        }
    }

    /// 处理秘密输入按键。
    ///
    /// 参数:
    /// - `code`: 键码
    /// - `modifiers`: 修饰键
    ///
    /// 返回:
    /// - 继续等待或最终应答
    fn handle_secret_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> PromptStep {
        match code {
            KeyCode::Enter => {
                let secret = std::mem::take(&mut self.secret);
                PromptStep::Done(SecretResponse::Provided(secret))
            }
            KeyCode::Esc => self.finish(SecretResponse::Cancelled),
            KeyCode::Backspace => {
                self.secret.pop();
                PromptStep::Continue
            }
            // Ctrl+U 清空整行，与 shell 行编辑习惯一致
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear_secret();
                PromptStep::Continue
            }
            KeyCode::Char(value) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.push_text(&value.to_string());
                PromptStep::Continue
            }
            _ => PromptStep::Continue,
        }
    }

    /// 处理是/否确认按键；默认拒绝。
    ///
    /// 参数:
    /// - `code`: 键码
    ///
    /// 返回:
    /// - 继续等待或最终应答
    fn handle_confirm_key(code: KeyCode) -> PromptStep {
        match code {
            KeyCode::Char('y' | 'Y') => PromptStep::Done(SecretResponse::Confirmed(true)),
            KeyCode::Char('n' | 'N') | KeyCode::Enter => {
                PromptStep::Done(SecretResponse::Confirmed(false))
            }
            KeyCode::Esc => PromptStep::Done(SecretResponse::Cancelled),
            _ => PromptStep::Continue,
        }
    }

    /// 追加输入文本，过滤控制字符并限制长度。
    ///
    /// 参数:
    /// - `text`: 键入或粘贴的文本
    ///
    /// 返回:
    /// - 无
    fn push_text(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            if self.secret.chars().count() >= MAX_SECRET_CHARS {
                break;
            }
            self.secret.push(ch);
        }
    }

    /// 以指定应答结束，并清空已输入的秘密。
    ///
    /// 参数:
    /// - `response`: 最终应答
    ///
    /// 返回:
    /// - 结束步骤
    fn finish(&mut self, response: SecretResponse) -> PromptStep {
        self.clear_secret();
        PromptStep::Done(response)
    }

    /// 覆写并清空秘密缓冲，缩短明文在内存中的停留时间。
    fn clear_secret(&mut self) {
        // clear 不释放容量，等长空白会写回同一块缓冲，覆盖原有明文字节
        let len = self.secret.len();
        self.secret.clear();
        self.secret.extend(std::iter::repeat_n(' ', len));
        self.secret.clear();
    }
}

impl Drop for PromptInput {
    /// 状态机销毁时清空残留秘密。
    fn drop(&mut self) {
        self.clear_secret();
    }
}

/// 判断征询类型是否需要输入秘密。
///
/// 参数:
/// - `kind`: 征询类型
///
/// 返回:
/// - 口令或密码类为 `true`
pub(super) fn is_secret_kind(kind: InteractiveKind) -> bool {
    matches!(
        kind,
        InteractiveKind::Passphrase | InteractiveKind::Password | InteractiveKind::SudoPassword
    )
}

/// 仅在请求仍等待时提交应答，避免与后端超时撤销竞态。
///
/// 参数:
/// - `request`: 交互征询
/// - `response`: 用户应答
///
/// 返回:
/// - 无
pub(super) fn submit_if_pending(request: &SecretRequest, response: SecretResponse) {
    if crate::ssh::is_pending(&request.id) {
        let _ = crate::ssh::submit_secret(&request.id, response);
    }
}

#[cfg(test)]
mod tests;

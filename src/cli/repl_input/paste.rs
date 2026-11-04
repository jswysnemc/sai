use crate::cli::repl_clipboard::ReplClipboardState;
use crate::cli::repl_windows_paste::{WindowsPasteKey, WindowsPasteState};
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Instant;

/// 【终端】【Windows 粘贴】优先整段插入剪贴板，匹配失败时保护连续输入中的控制键
/// 参数: state 为检测状态，clipboard 为附件，input 为正文，cursor 为字符光标，
/// code 和 modifiers 为当前按键
/// 返回: 按键已按粘贴处理时为真
pub(in crate::cli) fn handle_unbracketed_paste(
    state: &mut WindowsPasteState,
    clipboard: &mut ReplClipboardState,
    input: &mut String,
    cursor: &mut usize,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if !cfg!(windows) || !modifiers.is_empty() {
        return false;
    }
    let key = match code {
        KeyCode::Enter => WindowsPasteKey::Enter,
        KeyCode::Tab => WindowsPasteKey::Tab,
        _ => return false,
    };
    let now = Instant::now();
    // 1. 【终端】【Windows 粘贴】确认完整首行后沿用原子块和原生事件批量回放
    if code == KeyCode::Enter {
        let before_cursor: String = input.chars().take(*cursor).collect();
        if let Some(paste) = state.begin_from_system_clipboard(&before_cursor, now) {
            clipboard.replace_recent_text_with_paste(input, cursor, paste.prefix_chars, paste.text);
            return true;
        }
    }
    // 2. 【终端】【Windows 粘贴】剪贴板读取或匹配失败时，连续输入中的控制键仍属于正文
    state.insert_burst_control(input, cursor, key, now)
}

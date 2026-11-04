use crate::cli::repl_clipboard::ReplClipboardState;
use crate::cli::repl_windows_paste::{WindowsPasteKey, WindowsPasteState};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Instant;

/// 【终端】【粘贴事件】区分正文插入与已确认回放，原生队列只由真实输入循环消费
#[derive(Eq, PartialEq)]
enum PasteAction {
    Ignored,
    Replayed,
    Inserted,
}

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
    if !cfg!(windows) {
        return false;
    }
    let action = handle_key(
        state,
        clipboard,
        input,
        cursor,
        KeyEvent::new(code, modifiers),
        Instant::now(),
        crate::cli::repl_windows_paste::read_system_clipboard_text,
    );
    if action == PasteAction::Replayed {
        state.drain_console_replay();
    }
    action != PasteAction::Ignored
}

/// 【终端】【粘贴事件】统一消费回放、识别剪贴板和保护控制键，避免两种输入状态处理不同
/// 参数: state 为检测状态，clipboard 为附件，input 为正文，cursor 为字符光标，
/// event 为键盘事件，now 为接收时间，read 为剪贴板读取器
/// 返回: 普通事件、已确认回放或正文插入
fn handle_key(
    state: &mut WindowsPasteState,
    clipboard: &mut ReplClipboardState,
    input: &mut String,
    cursor: &mut usize,
    event: KeyEvent,
    now: Instant,
    read: impl FnOnce() -> Option<String>,
) -> PasteAction {
    if event.kind == KeyEventKind::Release {
        return PasteAction::Ignored;
    }
    let Some(key) = super::event_batch::windows_paste_key(event.code, event.modifiers) else {
        state.reset();
        return PasteAction::Ignored;
    };
    if state.consume_key_at(key, now) {
        return PasteAction::Replayed;
    }
    if key == WindowsPasteKey::LineFeed && !state.should_protect_control(now) {
        return PasteAction::Ignored;
    }
    // 1. 【终端】【Windows 粘贴】确认完整首行后沿用原子块和原生事件批量回放
    if matches!(key, WindowsPasteKey::Enter | WindowsPasteKey::LineFeed) {
        let before_cursor: String = input.chars().take(*cursor).collect();
        if let Some(paste) = state.begin_from_clipboard(&before_cursor, key, now, read) {
            clipboard.replace_recent_text_with_paste(input, cursor, paste.prefix_chars, paste.text);
            return PasteAction::Inserted;
        }
    }
    // 2. 【终端】【Windows 粘贴】剪贴板读取或匹配失败时，连续输入中的控制键仍属于正文
    if state.insert_burst_control(input, cursor, key, now) {
        PasteAction::Inserted
    } else {
        PasteAction::Ignored
    }
}

/// 【终端】【隔离回归】通过同一事件处理器验证提交行为，不读取测试调用者的原生队列
/// 参数: state 为检测状态，clipboard 为附件，input 为正文，cursor 为字符光标，
/// event 为键盘事件，now 为接收时间，read 为隔离剪贴板读取器
/// 返回: 当前事件属于粘贴时为真
#[cfg(test)]
fn handle_key_with_reader(
    state: &mut WindowsPasteState,
    clipboard: &mut ReplClipboardState,
    input: &mut String,
    cursor: &mut usize,
    event: KeyEvent,
    now: Instant,
    read: impl FnOnce() -> Option<String>,
) -> bool {
    handle_key(state, clipboard, input, cursor, event, now, read) != PasteAction::Ignored
}

#[cfg(test)]
#[path = "paste_tests.rs"]
mod tests;

//! 跨平台键盘事件规范化：消除 CapsLock、Shift 与终端编码差异导致的快捷键失效。

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

/// 【终端输入】【事件规范化】统一修饰键下的字母大小写与 Shift+Tab 表达。
///
/// Windows `ToUnicodeEx` 在 CapsLock 或 Shift 激活时会把 Ctrl/Alt 字母转为大写，
/// 导致仅匹配小写字符的快捷键全面失效；统一归为小写并保留原 `modifiers` 后，
/// 所有面板与输入框都能获得一致行为。
///
/// 参数:
/// - `event`: 原始终端事件
///
/// 返回:
/// - 规范化后的终端事件
pub(crate) fn normalize_event(event: Event) -> Event {
    match event {
        Event::Key(key) => Event::Key(normalize_key_event(key)),
        other => other,
    }
}

/// 【终端输入】【按键规范化】规范化单个按键事件。
///
/// 参数:
/// - `key`: 原始按键事件
///
/// 返回:
/// - 规范化后的按键事件
pub(crate) fn normalize_key_event(mut key: KeyEvent) -> KeyEvent {
    // 1. 【终端输入】【Shift+Tab】部分终端发送 Tab+SHIFT 而非 BackTab，统一为 BackTab
    if key.code == KeyCode::Tab && key.modifiers == KeyModifiers::SHIFT {
        key.code = KeyCode::BackTab;
        return key;
    }
    // 2. 【终端输入】【Ctrl/Alt 字母】保留原 modifiers，将 ASCII 大写字母归为小写避免 CapsLock/Shift 破坏匹配
    if key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    {
        if let KeyCode::Char(ch) = key.code {
            if ch.is_ascii_uppercase() {
                key.code = KeyCode::Char(ch.to_ascii_lowercase());
            }
        }
    }
    key
}

/// 【终端输入】【Ctrl 键匹配】大小写无关地判断是否为指定 Ctrl+<字母> 组合键（排除 Alt 干扰）。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
/// - `expected`: 目标 ASCII 小写字母
///
/// 返回:
/// - 命中时返回 true
pub(crate) fn is_ctrl_char(code: KeyCode, modifiers: KeyModifiers, expected: char) -> bool {
    modifiers.contains(KeyModifiers::CONTROL)
        && !modifiers.contains(KeyModifiers::ALT)
        && matches!(code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&expected))
}

/// 【终端输入】【Alt 键匹配】大小写无关地判断是否为指定 Alt+<字母> 组合键（排除 Ctrl 干扰，避免误伤 AltGr）。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
/// - `expected`: 目标 ASCII 小写字母
///
/// 返回:
/// - 命中时返回 true
pub(crate) fn is_alt_char(code: KeyCode, modifiers: KeyModifiers, expected: char) -> bool {
    modifiers.contains(KeyModifiers::ALT)
        && !modifiers.contains(KeyModifiers::CONTROL)
        && matches!(code, KeyCode::Char(ch) if ch.eq_ignore_ascii_case(&expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端输入测试】【大写归一化】Ctrl/Alt 组合键的大写字母归为小写，普通输入保留原样。
    #[test]
    fn normalizes_ctrl_and_alt_uppercase_without_touching_plain_text() {
        let ctrl_c = normalize_key_event(KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(ctrl_c.code, KeyCode::Char('c'));
        assert_eq!(ctrl_c.modifiers, KeyModifiers::CONTROL | KeyModifiers::SHIFT);

        let alt_o = normalize_key_event(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::ALT));
        assert_eq!(alt_o.code, KeyCode::Char('o'));
        assert_eq!(alt_o.modifiers, KeyModifiers::ALT);

        let plain_g = normalize_key_event(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert_eq!(plain_g.code, KeyCode::Char('G'));
    }

    /// 【终端输入测试】【Shift+Tab】仅 Shift+Tab 归为 BackTab，Ctrl+Tab 等组合保持不变。
    #[test]
    fn normalizes_shift_tab_to_backtab() {
        let shift_tab = normalize_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(shift_tab.code, KeyCode::BackTab);

        let ctrl_shift_tab = normalize_key_event(KeyEvent::new(
            KeyCode::Tab,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));
        assert_eq!(ctrl_shift_tab.code, KeyCode::Tab);
    }

    /// 【终端输入测试】【修饰键辅助】Ctrl 与 Alt 辅助函数大小写无关且互不误触发 AltGr。
    #[test]
    fn ctrl_and_alt_helpers_ignore_case_and_reject_altgr() {
        assert!(is_ctrl_char(KeyCode::Char('C'), KeyModifiers::CONTROL, 'c'));
        assert!(is_alt_char(KeyCode::Char('O'), KeyModifiers::ALT, 'o'));
        assert!(!is_alt_char(
            KeyCode::Char('o'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            'o'
        ));
        assert!(!is_ctrl_char(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            'c'
        ));
    }
}

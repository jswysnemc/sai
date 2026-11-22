//! 输入区共享快捷键：统一空闲与流式 composer 的换行、词级编辑、行级删除与宿主冲突回退键位。

use crate::cli::repl_text::{
    byte_index_for_char, remove_word_before_cursor, word_end_after, word_start_before,
};
use crate::cli::terminal_events::{is_alt_char, is_ctrl_char};
use crossterm::event::{KeyCode, KeyModifiers};

/// 共享编辑快捷键的执行结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli) enum ComposerEditOutcome {
    /// 仅移动了光标
    CursorMoved,
    /// 修改了输入文本
    TextEdited,
}

/// 【终端输入】【换行快捷键】判断是否应向输入框插入换行符。
///
/// 除 Shift+Enter 与 Ctrl+J 外，补充 Alt+Enter（兼容 Winc 等宿主或未启用 Kitty 协议的终端）。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - 命中换行快捷键时返回 true
pub(in crate::cli) fn is_composer_newline_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    let shift_or_alt_enter = code == KeyCode::Enter
        && (modifiers.contains(KeyModifiers::SHIFT) || modifiers.contains(KeyModifiers::ALT))
        && !modifiers.contains(KeyModifiers::CONTROL);
    shift_or_alt_enter || is_ctrl_char(code, modifiers, 'j')
}

/// 【终端输入】【全屏切换】Ctrl+O 或 Alt+O（规避 Winc VSCode 预设拦截 Ctrl+O）。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - 命中全屏切换键位时返回 true
pub(in crate::cli) fn is_fullscreen_toggle_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    is_ctrl_char(code, modifiers, 'o') || is_alt_char(code, modifiers, 'o')
}

/// 【终端输入】【折叠计划】Ctrl+T 或 Alt+T（规避 Winc IDEA 预设拦截 Ctrl+T）。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - 命中折叠计划面板键位时返回 true
pub(in crate::cli) fn is_todo_fold_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    is_ctrl_char(code, modifiers, 't') || is_alt_char(code, modifiers, 't')
}

/// 【终端输入】【外部编辑器】Ctrl+G 或 Alt+G（规避 Winc IDEA/VSCode 预设拦截 Ctrl+G）。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - 命中外部编辑器键位时返回 true
pub(in crate::cli) fn is_external_editor_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    is_ctrl_char(code, modifiers, 'g') || is_alt_char(code, modifiers, 'g')
}

/// 【终端输入】【强制重绘】Ctrl+L 或 Alt+L。
///
/// 参数:
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - 命中重绘键位时返回 true
pub(in crate::cli) fn is_redraw_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    is_ctrl_char(code, modifiers, 'l') || is_alt_char(code, modifiers, 'l')
}

/// 【终端输入】【输入编辑】执行空闲与流式 composer 共用的光标移动、删词与删行快捷键。
///
/// 覆盖：
/// - `Home` / `Ctrl+A`（移至开头）、`End` / `Ctrl+E`（移至末尾）
/// - `Ctrl+Left` / `Alt+Left` / `Alt+B`（前移一词）
/// - `Ctrl+Right` / `Alt+Right` / `Alt+F`（后移一词）
/// - `Ctrl+W` / `Ctrl+Backspace` / `Alt+Backspace`（删除前一词）
/// - `Ctrl+U`（删至当前行首，兼容 macOS Cmd+Backspace）
/// - `Ctrl+K`（删至当前行尾）
///
/// 参数:
/// - `input`: 输入缓冲区
/// - `cursor`: 光标字符位置
/// - `code`: 键码
/// - `modifiers`: 修饰键集合
///
/// 返回:
/// - 命中并执行时返回移动或编辑结果，否则返回 None
pub(in crate::cli) fn apply_composer_edit_key(
    input: &mut String,
    cursor: &mut usize,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Option<ComposerEditOutcome> {
    // 1. 【终端输入】【首尾移动】Home / Ctrl+A 与 End / Ctrl+E
    if code == KeyCode::Home || is_ctrl_char(code, modifiers, 'a') {
        *cursor = 0;
        return Some(ComposerEditOutcome::CursorMoved);
    }
    if code == KeyCode::End || is_ctrl_char(code, modifiers, 'e') {
        *cursor = input.chars().count();
        return Some(ComposerEditOutcome::CursorMoved);
    }
    // 2. 【终端输入】【词级移动】Ctrl/Alt+Left/Right 与 Winc/readline 常用的 Alt+B/Alt+F
    if (code == KeyCode::Left && modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT))
        || is_alt_char(code, modifiers, 'b')
    {
        *cursor = word_start_before(input, *cursor);
        return Some(ComposerEditOutcome::CursorMoved);
    }
    if (code == KeyCode::Right && modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT))
        || is_alt_char(code, modifiers, 'f')
    {
        *cursor = word_end_after(input, *cursor);
        return Some(ComposerEditOutcome::CursorMoved);
    }
    // 3. 【终端输入】【词级删除】Ctrl+W、Ctrl+Backspace 与 Alt+Backspace
    if is_ctrl_char(code, modifiers, 'w')
        || (code == KeyCode::Backspace
            && modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT))
    {
        remove_word_before_cursor(input, cursor);
        return Some(ComposerEditOutcome::TextEdited);
    }
    // 4. 【终端输入】【行级删除】Ctrl+U（删至当前行首）与 Ctrl+K（删至当前行尾）
    if is_ctrl_char(code, modifiers, 'u') {
        remove_to_line_start(input, cursor);
        return Some(ComposerEditOutcome::TextEdited);
    }
    if is_ctrl_char(code, modifiers, 'k') {
        remove_to_line_end(input, *cursor);
        return Some(ComposerEditOutcome::TextEdited);
    }
    None
}

/// 删除光标到当前逻辑行行首的文本；光标已在行首时若前方是换行符则删除该换行符。
///
/// 参数:
/// - `input`: 输入缓冲区
/// - `cursor`: 光标字符位置
///
/// 返回:
/// - 无
fn remove_to_line_start(input: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let chars: Vec<char> = input.chars().collect();
    let end = (*cursor).min(chars.len());
    let mut start = end;
    while start > 0 && chars[start - 1] != '\n' {
        start -= 1;
    }
    if start == end && start > 0 {
        start -= 1;
    }
    let byte_start = byte_index_for_char(input, start);
    let byte_end = byte_index_for_char(input, end);
    input.replace_range(byte_start..byte_end, "");
    *cursor = start;
}

/// 删除光标到当前逻辑行行尾的文本；光标已在行尾且后方是换行符时删除该换行符。
///
/// 参数:
/// - `input`: 输入缓冲区
/// - `cursor`: 光标字符位置
///
/// 返回:
/// - 无
fn remove_to_line_end(input: &mut String, cursor: usize) {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    if cursor >= len {
        return;
    }
    let mut end = cursor;
    while end < len && chars[end] != '\n' {
        end += 1;
    }
    if end == cursor && end < len {
        end += 1;
    }
    let byte_start = byte_index_for_char(input, cursor);
    let byte_end = byte_index_for_char(input, end);
    input.replace_range(byte_start..byte_end, "");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端测试】【词级编辑】验证 Alt+B/F、Ctrl/Alt+Backspace 与 Ctrl+U/K 在单行与多行下的行为。
    #[test]
    fn shared_composer_shortcuts_move_and_edit_words_and_lines() {
        let mut text = "alpha beta gamma".to_string();
        let mut cursor = text.chars().count();

        assert_eq!(
            apply_composer_edit_key(&mut text, &mut cursor, KeyCode::Char('b'), KeyModifiers::ALT),
            Some(ComposerEditOutcome::CursorMoved)
        );
        assert_eq!(cursor, 11);

        assert_eq!(
            apply_composer_edit_key(&mut text, &mut cursor, KeyCode::Char('f'), KeyModifiers::ALT),
            Some(ComposerEditOutcome::CursorMoved)
        );
        assert_eq!(cursor, 16);

        assert_eq!(
            apply_composer_edit_key(&mut text, &mut cursor, KeyCode::Backspace, KeyModifiers::ALT),
            Some(ComposerEditOutcome::TextEdited)
        );
        assert_eq!(text, "alpha beta ");
        assert_eq!(cursor, 11);

        assert_eq!(
            apply_composer_edit_key(
                &mut text,
                &mut cursor,
                KeyCode::Backspace,
                KeyModifiers::CONTROL
            ),
            Some(ComposerEditOutcome::TextEdited)
        );
        assert_eq!(text, "alpha ");
        assert_eq!(cursor, 6);

        let mut multiline = "first\nsecond line".to_string();
        let mut mcursor = 12;
        assert_eq!(
            apply_composer_edit_key(
                &mut multiline,
                &mut mcursor,
                KeyCode::Char('u'),
                KeyModifiers::CONTROL
            ),
            Some(ComposerEditOutcome::TextEdited)
        );
        assert_eq!(multiline, "first\n line");
        assert_eq!(mcursor, 6);

        assert_eq!(
            apply_composer_edit_key(
                &mut multiline,
                &mut mcursor,
                KeyCode::Char('k'),
                KeyModifiers::CONTROL
            ),
            Some(ComposerEditOutcome::TextEdited)
        );
        assert_eq!(multiline, "first\n");
    }

    /// 【终端测试】【宿主冲突回退】验证 Shift/Alt+Enter、Ctrl+J 及 Alt+O/T/G/L 回退键位。
    #[test]
    fn recognizes_newline_and_alt_fallback_shortcuts() {
        assert!(is_composer_newline_key(KeyCode::Enter, KeyModifiers::SHIFT));
        assert!(is_composer_newline_key(KeyCode::Enter, KeyModifiers::ALT));
        assert!(is_composer_newline_key(
            KeyCode::Char('j'),
            KeyModifiers::CONTROL
        ));
        assert!(!is_composer_newline_key(
            KeyCode::Enter,
            KeyModifiers::CONTROL
        ));
        assert!(is_fullscreen_toggle_key(
            KeyCode::Char('o'),
            KeyModifiers::ALT
        ));
        assert!(is_todo_fold_key(KeyCode::Char('t'), KeyModifiers::ALT));
        assert!(is_external_editor_key(
            KeyCode::Char('g'),
            KeyModifiers::ALT
        ));
        assert!(is_redraw_key(KeyCode::Char('l'), KeyModifiers::ALT));
    }
}

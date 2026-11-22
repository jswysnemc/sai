//! 跨平台终端转义序列解码器：解析 Kitty CSI-u、xterm modifyOtherKeys、标准 CSI/SS3，并丢弃终端能力查询残留应答。

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// 单个转义序列的解码结果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EscapeOutcome {
    /// 成功解析为标准终端事件
    Event(Event),
    /// 终端查询残留应答、括号粘贴标记或纯修饰键，静默丢弃不污染输入
    Discard,
    /// 未识别的转义序列，需由调用方按原始按键回放
    Unrecognized,
}

/// 【终端输入】【转义解码】将完整的 ESC 序列（CSI / SS3）解码为标准 crossterm 事件。
///
/// 参数:
/// - `sequence`: 以 `\x1b` 开头的完整转义序列
///
/// 返回:
/// - 解码后的事件、静默丢弃标记或未识别标记
pub(crate) fn parse_escape_sequence(sequence: &str) -> EscapeOutcome {
    if sequence == "\x1b[I" {
        return EscapeOutcome::Event(Event::FocusGained);
    }
    if sequence == "\x1b[O" {
        return EscapeOutcome::Event(Event::FocusLost);
    }
    if let Some(body) = sequence.strip_prefix("\x1b[") {
        return parse_csi(body);
    }
    if let Some(body) = sequence.strip_prefix("\x1bO") {
        return parse_ss3(body);
    }
    EscapeOutcome::Unrecognized
}

/// 解析 CSI 序列正文（不含 `\x1b[`）。
///
/// 参数:
/// - `body`: CSI 参数与终结字节
///
/// 返回:
/// - 解码结果
fn parse_csi(body: &str) -> EscapeOutcome {
    let Some(final_byte) = body.bytes().last() else {
        return EscapeOutcome::Unrecognized;
    };
    let params = &body[..body.len() - 1];

    // 1. 【终端输入】【应答清理】过滤延迟到达的 Kitty 键盘查询、DA1/DA2、CSI 16t 及光标报告
    if is_terminal_query_reply(params, final_byte) {
        return EscapeOutcome::Discard;
    }

    match final_byte {
        b'u' => parse_kitty_csi_u(params),
        b'~' => parse_csi_tilde(params),
        b'A' | b'B' | b'C' | b'D' | b'H' | b'F' | b'P' | b'Q' | b'R' | b'S' | b'Z' => {
            parse_csi_final(params, final_byte)
        }
        _ => EscapeOutcome::Unrecognized,
    }
}

/// 判断 CSI 序列是否属于终端能力或状态查询应答。
///
/// 参数:
/// - `params`: 终结字节之前的参数段
/// - `final_byte`: CSI 终结字节
///
/// 返回:
/// - 属于终端查询应答时返回 true
fn is_terminal_query_reply(params: &str, final_byte: u8) -> bool {
    match final_byte {
        // Kitty 键盘能力应答 \x1b[?<flags>u
        b'u' => {
            params.starts_with('?')
                && params[1..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
        }
        // DA1 / DA2 / DA3 应答 \x1b[?...c / \x1b[>...c / \x1b[=...c
        b'c' => {
            params.starts_with(['?', '>', '='])
                && params[1..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
        }
        // CSI 14t / 16t / 18t 窗口或单元格像素报告 \x1b[4;...t / \x1b[6;...t / \x1b[8;...t
        b't' => {
            (params.starts_with("4;") || params.starts_with("6;") || params.starts_with("8;"))
                && params
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
        }
        // 扩展光标位置报告 \x1b[?row;colR
        b'R' => {
            params.starts_with('?')
                && params[1..]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || byte == b';')
        }
        _ => false,
    }
}

/// 【终端输入】【Kitty CSI-u】解析 `code[:shifted[:base]][;modifiers[:event_type]][;text]u`。
///
/// 参数:
/// - `params`: 去掉 `\x1b[` 与末尾 `u` 后的参数段
///
/// 返回:
/// - 解码后的按键事件或丢弃/未识别标记
fn parse_kitty_csi_u(params: &str) -> EscapeOutcome {
    if params.is_empty() || params.starts_with(['?', '>', '<', '=']) {
        return EscapeOutcome::Unrecognized;
    }
    let mut sections = params.split(';');
    let Some(key_section) = sections.next() else {
        return EscapeOutcome::Unrecognized;
    };
    let mod_section = sections.next();

    // 1. 解析主码点、Shift 变体码点与基础布局码点
    let mut key_parts = key_section.split(':');
    let Ok(codepoint) = key_parts.next().unwrap_or_default().parse::<u32>() else {
        return EscapeOutcome::Unrecognized;
    };
    let shifted_cp = key_parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0);
    let base_cp = key_parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0);

    // 2. 解析修饰键位图与事件类型（1=Press, 2=Repeat, 3=Release）
    let (modifiers, mut state, kind) = parse_modifier_and_kind(mod_section);

    // 3. 映射功能键、特殊控制键与 Unicode 字符键
    let Some((code, extra_state)) =
        map_kitty_codepoint(codepoint, shifted_cp, base_cp, modifiers)
    else {
        return if (57441..=57454).contains(&codepoint) {
            EscapeOutcome::Discard
        } else {
            EscapeOutcome::Unrecognized
        };
    };
    state |= extra_state;
    EscapeOutcome::Event(Event::Key(KeyEvent {
        code,
        modifiers,
        kind,
        state,
    }))
}

/// 将 Kitty CSI-u 码点映射为 crossterm `KeyCode`。
///
/// 参数:
/// - `codepoint`: 主键 Unicode 码点或 Kitty PUA 功能键码点
/// - `shifted_cp`: 可选 Shift 变体码点
/// - `base_cp`: 可选基础布局码点
/// - `modifiers`: 已解码的修饰键集合
///
/// 返回:
/// - 对应的 `(KeyCode, KeyEventState)`；修饰键本身或未知码点返回 None
fn map_kitty_codepoint(
    codepoint: u32,
    shifted_cp: Option<u32>,
    base_cp: Option<u32>,
    modifiers: KeyModifiers,
) -> Option<(KeyCode, KeyEventState)> {
    let code = match codepoint {
        27 => return Some((KeyCode::Esc, KeyEventState::NONE)),
        13 => return Some((KeyCode::Enter, KeyEventState::NONE)),
        9 => {
            if modifiers.contains(KeyModifiers::SHIFT)
                && !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            {
                return Some((KeyCode::BackTab, KeyEventState::NONE));
            }
            return Some((KeyCode::Tab, KeyEventState::NONE));
        }
        127 | 8 => return Some((KeyCode::Backspace, KeyEventState::NONE)),
        57361 => return Some((KeyCode::PrintScreen, KeyEventState::NONE)),
        57362 => return Some((KeyCode::Pause, KeyEventState::NONE)),
        57363 => return Some((KeyCode::Menu, KeyEventState::NONE)),
        57376..=57398 => {
            return Some((
                KeyCode::F((codepoint - 57376 + 13) as u8),
                KeyEventState::NONE,
            ))
        }
        57399..=57408 => {
            let digit = char::from_u32(u32::from(b'0') + (codepoint - 57399))?;
            return Some((KeyCode::Char(digit), KeyEventState::KEYPAD));
        }
        57409 => return Some((KeyCode::Char('.'), KeyEventState::KEYPAD)),
        57410 => return Some((KeyCode::Char('/'), KeyEventState::KEYPAD)),
        57411 => return Some((KeyCode::Char('*'), KeyEventState::KEYPAD)),
        57412 => return Some((KeyCode::Char('-'), KeyEventState::KEYPAD)),
        57413 => return Some((KeyCode::Char('+'), KeyEventState::KEYPAD)),
        57414 => return Some((KeyCode::Enter, KeyEventState::KEYPAD)),
        57415 => return Some((KeyCode::Char('='), KeyEventState::KEYPAD)),
        57417 => return Some((KeyCode::Left, KeyEventState::KEYPAD)),
        57418 => return Some((KeyCode::Right, KeyEventState::KEYPAD)),
        57419 => return Some((KeyCode::Up, KeyEventState::KEYPAD)),
        57420 => return Some((KeyCode::Down, KeyEventState::KEYPAD)),
        57421 => return Some((KeyCode::PageUp, KeyEventState::KEYPAD)),
        57422 => return Some((KeyCode::PageDown, KeyEventState::KEYPAD)),
        57423 => return Some((KeyCode::Home, KeyEventState::KEYPAD)),
        57424 => return Some((KeyCode::End, KeyEventState::KEYPAD)),
        57425 => return Some((KeyCode::Insert, KeyEventState::KEYPAD)),
        57426 => return Some((KeyCode::Delete, KeyEventState::KEYPAD)),
        57441..=57454 => return None,
        _ => {
            let mut ch = char::from_u32(codepoint)?;
            let has_ctrl_or_alt = modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
            if has_ctrl_or_alt {
                // 非 ASCII 布局下按下 Ctrl/Alt 快捷键时优先使用基础 ASCII 布局键
                if !ch.is_ascii() {
                    if let Some(base) = base_cp.and_then(char::from_u32).filter(char::is_ascii) {
                        ch = base;
                    }
                }
                if ch.is_ascii_uppercase() {
                    ch = ch.to_ascii_lowercase();
                }
            } else if modifiers.contains(KeyModifiers::SHIFT) {
                if let Some(shifted) = shifted_cp.and_then(char::from_u32) {
                    ch = shifted;
                } else if ch.is_ascii_lowercase() {
                    ch = ch.to_ascii_uppercase();
                }
            }
            KeyCode::Char(ch)
        }
    };
    Some((code, KeyEventState::NONE))
}

/// 解析波浪号终结的 CSI 序列（含 xterm modifyOtherKeys 与功能键）。
///
/// 参数:
/// - `params`: 去掉 `\x1b[` 与 `~` 后的参数段
///
/// 返回:
/// - 解码结果
fn parse_csi_tilde(params: &str) -> EscapeOutcome {
    let parts: Vec<&str> = params.split(';').collect();
    let Ok(first) = parts.first().copied().unwrap_or_default().parse::<u32>() else {
        return EscapeOutcome::Unrecognized;
    };
    // 1. 括号粘贴起止标记不泄露为普通输入
    if first == 200 || first == 201 {
        return EscapeOutcome::Discard;
    }
    // 2. xterm modifyOtherKeys 格式：\x1b[27;<mod>;<code>~
    if first == 27 && parts.len() == 3 {
        let Ok(codepoint) = parts[2].parse::<u32>() else {
            return EscapeOutcome::Unrecognized;
        };
        let (modifiers, state, kind) = parse_modifier_and_kind(Some(parts[1]));
        let Some((code, extra_state)) = map_kitty_codepoint(codepoint, None, None, modifiers)
        else {
            return EscapeOutcome::Unrecognized;
        };
        return EscapeOutcome::Event(Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            state: state | extra_state,
        }));
    }
    let code = match first {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11..=15 => KeyCode::F((first - 10) as u8),
        17..=21 => KeyCode::F((first - 11) as u8),
        23..=24 => KeyCode::F((first - 12) as u8),
        _ => return EscapeOutcome::Unrecognized,
    };
    let (modifiers, state, kind) = parse_modifier_and_kind(parts.get(1).copied());
    EscapeOutcome::Event(Event::Key(KeyEvent {
        code,
        modifiers,
        kind,
        state,
    }))
}

/// 解析以字母终结的标准 CSI 方向键与功能键序列。
///
/// 参数:
/// - `params`: 参数段
/// - `final_byte`: 终结字母
///
/// 返回:
/// - 解码结果
fn parse_csi_final(params: &str, final_byte: u8) -> EscapeOutcome {
    if final_byte == b'Z' {
        let (mut modifiers, state, kind) = if params.is_empty() {
            (KeyModifiers::SHIFT, KeyEventState::NONE, KeyEventKind::Press)
        } else {
            let mut parts = params.split(';');
            let _ = parts.next();
            parse_modifier_and_kind(parts.next())
        };
        modifiers |= KeyModifiers::SHIFT;
        return EscapeOutcome::Event(Event::Key(KeyEvent {
            code: KeyCode::BackTab,
            modifiers,
            kind,
            state,
        }));
    }
    let (first_num, mod_part) = if params.is_empty() {
        (None, None)
    } else {
        let mut parts = params.split(';');
        let first = parts.next().and_then(|value| value.parse::<u32>().ok());
        let second = parts.next();
        if parts.next().is_some() {
            return EscapeOutcome::Unrecognized;
        }
        (first, second)
    };
    // 1. 区分光标位置报告 \x1b[row;colR 与带修饰的 F3 \x1b[1;modR
    if final_byte == b'R' {
        if first_num == Some(1) && mod_part.is_some() {
            let (modifiers, state, kind) = parse_modifier_and_kind(mod_part);
            return EscapeOutcome::Event(Event::Key(KeyEvent {
                code: KeyCode::F(3),
                modifiers,
                kind,
                state,
            }));
        }
        if first_num.is_some() && mod_part.is_some() {
            return EscapeOutcome::Discard;
        }
        return EscapeOutcome::Unrecognized;
    }
    if first_num.is_some_and(|value| value != 1) {
        return EscapeOutcome::Unrecognized;
    }
    let code = match final_byte {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P' => KeyCode::F(1),
        b'Q' => KeyCode::F(2),
        b'S' => KeyCode::F(4),
        _ => return EscapeOutcome::Unrecognized,
    };
    let (modifiers, state, kind) = parse_modifier_and_kind(mod_part);
    EscapeOutcome::Event(Event::Key(KeyEvent {
        code,
        modifiers,
        kind,
        state,
    }))
}

/// 解析 SS3 序列（`\x1bO<ch>`）。
///
/// 参数:
/// - `body`: 去掉 `\x1bO` 后的字符
///
/// 返回:
/// - 解码结果
fn parse_ss3(body: &str) -> EscapeOutcome {
    let code = match body {
        "A" => KeyCode::Up,
        "B" => KeyCode::Down,
        "C" => KeyCode::Right,
        "D" => KeyCode::Left,
        "H" => KeyCode::Home,
        "F" => KeyCode::End,
        "P" => KeyCode::F(1),
        "Q" => KeyCode::F(2),
        "R" => KeyCode::F(3),
        "S" => KeyCode::F(4),
        _ => return EscapeOutcome::Unrecognized,
    };
    EscapeOutcome::Event(Event::Key(KeyEvent::new(code, KeyModifiers::NONE)))
}

/// 解析 Kitty/xterm 修饰键参数与可选事件类型（`modifiers[:event_type]`）。
///
/// 参数:
/// - `section`: 可选修饰键参数段
///
/// 返回:
/// - `(修饰键集合, 锁状态, 事件类型)`
fn parse_modifier_and_kind(section: Option<&str>) -> (KeyModifiers, KeyEventState, KeyEventKind) {
    let Some(section) = section.filter(|value| !value.is_empty()) else {
        return (KeyModifiers::NONE, KeyEventState::NONE, KeyEventKind::Press);
    };
    let mut parts = section.split(':');
    let raw = parts
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1)
        .saturating_sub(1);
    let kind = match parts.next().and_then(|value| value.parse::<u8>().ok()) {
        Some(2) => KeyEventKind::Repeat,
        Some(3) => KeyEventKind::Release,
        _ => KeyEventKind::Press,
    };
    let mut modifiers = KeyModifiers::NONE;
    if raw & 1 != 0 {
        modifiers |= KeyModifiers::SHIFT;
    }
    if raw & 2 != 0 {
        modifiers |= KeyModifiers::ALT;
    }
    if raw & 4 != 0 {
        modifiers |= KeyModifiers::CONTROL;
    }
    if raw & 8 != 0 {
        modifiers |= KeyModifiers::SUPER;
    }
    if raw & 16 != 0 {
        modifiers |= KeyModifiers::HYPER;
    }
    if raw & 32 != 0 {
        modifiers |= KeyModifiers::META;
    }
    let mut state = KeyEventState::NONE;
    if raw & 64 != 0 {
        state |= KeyEventState::CAPS_LOCK;
    }
    if raw & 128 != 0 {
        state |= KeyEventState::NUM_LOCK;
    }
    (modifiers, state, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端输入测试】【Kitty CSI-u】覆盖 Shift/Ctrl/Alt+Enter、Esc、Shift+Tab 与 Ctrl/Alt 组合键。
    #[test]
    fn decodes_kitty_csi_u_shortcuts() {
        assert_eq!(
            parse_escape_sequence("\x1b[13;2u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::SHIFT
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[13;5u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::CONTROL
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[13;3u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::ALT
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[27u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[9;2u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::BackTab,
                KeyModifiers::SHIFT
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[99;5u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[127;5u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Backspace,
                KeyModifiers::CONTROL
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[127;3u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Backspace,
                KeyModifiers::ALT
            )))
        );
    }

    /// 【终端输入测试】【事件类型与变体】支持 Repeat/Release 事件类型及大写或基础布局变体。
    #[test]
    fn decodes_kitty_event_kinds_and_alternate_keys() {
        assert_eq!(
            parse_escape_sequence("\x1b[99;5:3u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
                KeyEventKind::Release
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[99:67;6u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[47:63;2u"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Char('?'),
                KeyModifiers::SHIFT
            )))
        );
    }

    /// 【终端输入测试】【xterm 兼容】支持 modifyOtherKeys、方向键、波浪键与 SS3。
    #[test]
    fn decodes_xterm_modify_other_keys_and_csi_navigation() {
        assert_eq!(
            parse_escape_sequence("\x1b[27;2;13~"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::SHIFT
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[1;5A"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::Up,
                KeyModifiers::CONTROL
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[Z"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::BackTab,
                KeyModifiers::SHIFT
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1b[5~"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::PageUp,
                KeyModifiers::NONE
            )))
        );
        assert_eq!(
            parse_escape_sequence("\x1bOP"),
            EscapeOutcome::Event(Event::Key(KeyEvent::new(
                KeyCode::F(1),
                KeyModifiers::NONE
            )))
        );
    }

    /// 【终端输入测试】【应答丢弃】延迟的终端能力应答与括号粘贴标记静默丢弃，不泄露到输入框。
    #[test]
    fn discards_terminal_query_replies_and_bracketed_paste_markers() {
        for seq in [
            "\x1b[?0u",
            "\x1b[?1u",
            "\x1b[?61;4;6;22c",
            "\x1b[>0;10;1c",
            "\x1b[6;24;12t",
            "\x1b[200~",
            "\x1b[201~",
            "\x1b[57441u",
        ] {
            assert_eq!(parse_escape_sequence(seq), EscapeOutcome::Discard, "{seq}");
        }
    }
}

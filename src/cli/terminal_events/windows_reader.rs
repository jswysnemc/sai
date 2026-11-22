//! Windows ConPTY 原生输入读取器：在 crossterm 之前截获 VK=0 ESC 引导的 CSI-u / xterm 序列。
//!
//! crossterm 0.28 在 Windows 上遇到 `wVirtualKeyCode == 0, uChar == 0x1b` 时会直接丢弃 ESC，
//! 导致 Kitty 键盘协议（如 ELYTerminal / Winc 的 `\x1b[13;2u`、`\x1b[99;5u`、`\x1b[27u`）
//! 变成乱码字符。本模块通过 `PeekConsoleInputW` 仅接管 ConPTY 直通的 ESC 序列，
//! 普通按键、鼠标、缩放、焦点与中文 IME 代理对仍原样交由 crossterm 处理。

#[cfg(any(windows, test))]
use super::csi_u::{parse_escape_sequence, EscapeOutcome};
#[cfg(any(windows, test))]
use super::normalize::normalize_event;
#[cfg(any(windows, test))]
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
#[cfg(windows)]
use std::collections::VecDeque;
#[cfg(windows)]
use std::io;
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
const MAX_ESCAPE_CHARS: usize = 64;
#[cfg(windows)]
const ESCAPE_FOLLOW_TIMEOUT: Duration = Duration::from_millis(30);

#[cfg(windows)]
static PENDING_EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());

/// 【终端输入】【序列回退】将未能识别的原始转义序列拆回可回放按键，确保不丢失用户输入。
///
/// 参数:
/// - `sequence`: 以 `\x1b` 开头的原始字符序列
///
/// 返回:
/// - 回放给上层的标准事件列表
#[cfg(any(windows, test))]
fn fallback_events_for_escape(sequence: &str) -> Vec<Event> {
    let mut chars = sequence.chars();
    let Some(first) = chars.next() else {
        return Vec::new();
    };
    if first != '\x1b' {
        return sequence
            .chars()
            .map(|ch| Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)))
            .collect();
    }
    let rest: Vec<char> = chars.collect();
    if rest.is_empty() {
        return vec![Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))];
    }
    if rest.len() == 1 && rest[0] != '[' && rest[0] != 'O' {
        return vec![normalize_event(Event::Key(KeyEvent::new(
            KeyCode::Char(rest[0]),
            KeyModifiers::ALT,
        )))];
    }
    let mut events = Vec::with_capacity(rest.len() + 1);
    events.push(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    for ch in rest {
        events.push(Event::Key(KeyEvent::new(
            KeyCode::Char(ch),
            KeyModifiers::NONE,
        )));
    }
    events
}

/// 【终端输入】【序列定界】将一条已读取的原始转义序列转换为零到多个规范化事件。
///
/// 参数:
/// - `sequence`: 完整或超时的转义字符序列
///
/// 返回:
/// - 应当入队交付上层的事件列表
#[cfg(any(windows, test))]
fn decode_collected_escape(sequence: &str) -> Vec<Event> {
    match parse_escape_sequence(sequence) {
        EscapeOutcome::Event(event) => vec![normalize_event(event)],
        EscapeOutcome::Discard => Vec::new(),
        EscapeOutcome::Unrecognized => fallback_events_for_escape(sequence),
    }
}

/// 【终端输入】【Windows 轮询】等待并预解析下一条可用事件，避免 crossterm::event::poll 提前吃掉 VK=0 ESC。
///
/// 参数:
/// - `timeout`: 最大等待时间
///
/// 返回:
/// - 是否存在可立即读取的事件
#[cfg(windows)]
pub(super) fn poll(timeout: Duration) -> io::Result<bool> {
    if has_pending_event() {
        return Ok(true);
    }
    pump_until(Some(timeout))
}

/// 【终端输入】【Windows 读取】返回下一条规范化终端事件，阻塞直至有事件就绪。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 规范化后的终端事件
#[cfg(windows)]
pub(super) fn read() -> io::Result<Event> {
    loop {
        if let Some(event) = pop_pending_event() {
            return Ok(event);
        }
        pump_until(None)?;
    }
}

#[cfg(windows)]
fn has_pending_event() -> bool {
    !PENDING_EVENTS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .is_empty()
}

#[cfg(windows)]
fn pop_pending_event() -> Option<Event> {
    PENDING_EVENTS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .pop_front()
}

#[cfg(windows)]
fn push_pending_events(events: impl IntoIterator<Item = Event>) {
    let mut queue = PENDING_EVENTS
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    queue.extend(events);
}

/// 【终端输入】【Windows 泵送】等待控制台输入并解析一条事件放入内部队列。
///
/// 参数:
/// - `timeout`: 可选超时时间；为 None 时阻塞直到得到一条事件
///
/// 返回:
/// - 是否成功产出至少一条待读事件
#[cfg(windows)]
fn pump_until(timeout: Option<Duration>) -> io::Result<bool> {
    use windows_sys::Win32::Foundation::{
        HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Console::{
        GetNumberOfConsoleInputEvents, GetStdHandle, PeekConsoleInputW, ReadConsoleInputW,
        INPUT_RECORD, KEY_EVENT, STD_INPUT_HANDLE,
    };
    use windows_sys::Win32::System::Threading::{WaitForSingleObject, INFINITE};

    let handle: HANDLE = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return fallback_crossterm_pump(timeout);
    }
    let mut check_count = 0u32;
    if unsafe { GetNumberOfConsoleInputEvents(handle, &mut check_count) } == 0 {
        return fallback_crossterm_pump(timeout);
    }

    let deadline = timeout.map(|duration| Instant::now() + duration);
    loop {
        if has_pending_event() {
            return Ok(true);
        }
        let mut peek_buf = [INPUT_RECORD::default(); 4];
        let mut peeked = 0u32;
        if unsafe {
            PeekConsoleInputW(
                handle,
                peek_buf.as_mut_ptr(),
                peek_buf.len() as u32,
                &mut peeked,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }

        if peeked == 0 {
            let wait_ms = match deadline {
                None => INFINITE,
                Some(end) => {
                    let Some(remaining) = end.checked_duration_since(Instant::now()) else {
                        return Ok(false);
                    };
                    if remaining.is_zero() {
                        return Ok(false);
                    }
                    remaining.as_millis().clamp(1, u128::from(u32::MAX - 1)) as u32
                }
            };
            match unsafe { WaitForSingleObject(handle, wait_ms) } {
                WAIT_OBJECT_0 => continue,
                WAIT_TIMEOUT => return Ok(false),
                _ => return Err(io::Error::last_os_error()),
            }
        }

        let slice = &peek_buf[..peeked as usize];
        // 1. 【终端输入】【CSI-u 拦截】检测到 ConPTY 直通的 VK=0 ESC 或成批注入的 ESC+[ 序列时接管解析
        if is_raw_escape_start(slice) {
            let sequence = read_raw_escape_sequence(handle)?;
            let decoded = decode_collected_escape(&sequence);
            if !decoded.is_empty() {
                push_pending_events(decoded);
                return Ok(true);
            }
            continue;
        }
        // 2. 【终端输入】【残留释放】清理前序原始序列遗留的 VK=0 按键释放记录
        if slice[0].EventType == KEY_EVENT as u16 {
            let key = unsafe { slice[0].Event.KeyEvent };
            if key.bKeyDown == 0 && key.wVirtualKeyCode == 0 {
                let mut dummy = INPUT_RECORD::default();
                let mut read = 0u32;
                let _ = unsafe { ReadConsoleInputW(handle, &mut dummy, 1, &mut read) };
                continue;
            }
        }
        // 3. 【终端输入】【普通事件】单条零超时交由 crossterm 解析，避免越过本条记录误吞后续 VK=0 ESC
        if crossterm::event::poll(Duration::ZERO)? {
            let event = normalize_event(crossterm::event::read()?);
            push_pending_events([event]);
            return Ok(true);
        }
        if let Some(end) = deadline {
            if Instant::now() >= end {
                return Ok(false);
            }
        }
    }
}

/// 非控制台句柄回退到 crossterm 默认实现。
#[cfg(windows)]
fn fallback_crossterm_pump(timeout: Option<Duration>) -> io::Result<bool> {
    match timeout {
        Some(duration) => {
            if crossterm::event::poll(duration)? {
                push_pending_events([normalize_event(crossterm::event::read()?)]);
                Ok(true)
            } else {
                Ok(false)
            }
        }
        None => {
            push_pending_events([normalize_event(crossterm::event::read()?)]);
            Ok(true)
        }
    }
}

/// 判断队列头部记录是否为 ConPTY 直通的转义序列起点。
#[cfg(windows)]
fn is_raw_escape_start(records: &[windows_sys::Win32::System::Console::INPUT_RECORD]) -> bool {
    use windows_sys::Win32::System::Console::KEY_EVENT;

    let Some(first) = records.first() else {
        return false;
    };
    if first.EventType != KEY_EVENT as u16 {
        return false;
    }
    let key = unsafe { first.Event.KeyEvent };
    let ch = unsafe { key.uChar.UnicodeChar };
    if key.bKeyDown == 0 || ch != 0x1b || key.dwControlKeyState != 0 {
        return false;
    }
    // 1. ConPTY WriteStringRaw 总是以 wVirtualKeyCode == 0 写入 ESC
    if key.wVirtualKeyCode == 0 {
        return true;
    }
    // 2. 宿主通过 sendInput 批量注入 CSI（VK_ESCAPE 后紧邻 '['）
    if key.wVirtualKeyCode == 0x1b {
        for next in records.iter().skip(1) {
            if next.EventType != KEY_EVENT as u16 {
                break;
            }
            let next_key = unsafe { next.Event.KeyEvent };
            if next_key.bKeyDown == 0 {
                continue;
            }
            let next_ch = unsafe { next_key.uChar.UnicodeChar };
            return next_ch == u16::from(b'[') && next_key.dwControlKeyState == 0;
        }
    }
    false
}

/// 从控制台队列读取一条完整的 ESC/CSI/SS3 序列，不消耗不属于该序列的后续记录。
#[cfg(windows)]
fn read_raw_escape_sequence(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> io::Result<String> {
    use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows_sys::Win32::System::Console::{
        PeekConsoleInputW, ReadConsoleInputW, INPUT_RECORD, KEY_EVENT,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    let mut sequence = String::with_capacity(16);
    let mut record = INPUT_RECORD::default();
    let mut read = 0u32;
    if unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if read == 0 {
        return Ok(sequence);
    }
    sequence.push('\x1b');
    let mut last_unit = 0x1bu16;
    let deadline = Instant::now() + ESCAPE_FOLLOW_TIMEOUT;

    while sequence.len() < MAX_ESCAPE_CHARS {
        let mut peek = INPUT_RECORD::default();
        let mut peeked = 0u32;
        if unsafe { PeekConsoleInputW(handle, &mut peek, 1, &mut peeked) } == 0 {
            return Err(io::Error::last_os_error());
        }
        if peeked == 0 {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            let wait_ms = remaining.as_millis().clamp(1, 30) as u32;
            match unsafe { WaitForSingleObject(handle, wait_ms) } {
                WAIT_OBJECT_0 => continue,
                WAIT_TIMEOUT => break,
                _ => return Err(io::Error::last_os_error()),
            }
        }
        if peek.EventType != KEY_EVENT as u16 {
            break;
        }
        let key = unsafe { peek.Event.KeyEvent };
        let unit = unsafe { key.uChar.UnicodeChar };
        // 1. 吞掉当前序列字符成对到达的 key-up 记录
        if key.bKeyDown == 0 {
            if unit == last_unit || key.wVirtualKeyCode == 0 {
                let _ = unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) };
                continue;
            }
            break;
        }
        if key.dwControlKeyState != 0 {
            break;
        }
        let Some(byte) = u8::try_from(unit).ok() else {
            break;
        };
        if sequence.len() == 1 {
            if byte == b'[' || byte == b'O' {
                let _ = unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) };
                sequence.push(byte as char);
                last_unit = unit;
                continue;
            }
            break;
        }
        if sequence.starts_with("\x1bO") {
            if (0x21..=0x7e).contains(&byte) {
                let _ = unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) };
                sequence.push(byte as char);
                consume_matching_release(handle, unit);
            }
            break;
        }
        if sequence.starts_with("\x1b[") {
            if (0x20..=0x3f).contains(&byte) {
                let _ = unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) };
                sequence.push(byte as char);
                last_unit = unit;
                continue;
            }
            if (0x40..=0x7e).contains(&byte) {
                let _ = unsafe { ReadConsoleInputW(handle, &mut record, 1, &mut read) };
                sequence.push(byte as char);
                consume_matching_release(handle, unit);
            }
            break;
        }
        break;
    }
    Ok(sequence)
}

/// 立即清理紧跟在终结字节后的同字符 key-up 记录（非阻塞）。
#[cfg(windows)]
fn consume_matching_release(handle: windows_sys::Win32::Foundation::HANDLE, expected_unit: u16) {
    use windows_sys::Win32::System::Console::{
        PeekConsoleInputW, ReadConsoleInputW, INPUT_RECORD, KEY_EVENT,
    };

    let mut peek = INPUT_RECORD::default();
    let mut peeked = 0u32;
    if unsafe { PeekConsoleInputW(handle, &mut peek, 1, &mut peeked) } != 0 && peeked == 1 {
        if peek.EventType == KEY_EVENT as u16 {
            let key = unsafe { peek.Event.KeyEvent };
            let unit = unsafe { key.uChar.UnicodeChar };
            if key.bKeyDown == 0 && (unit == expected_unit || key.wVirtualKeyCode == 0) {
                let mut dummy = INPUT_RECORD::default();
                let mut read = 0u32;
                let _ = unsafe { ReadConsoleInputW(handle, &mut dummy, 1, &mut read) };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端输入测试】【回退保底】单 ESC 与未知序列按顺序还原，不丢失按键。
    #[test]
    fn unrecognized_and_bare_escape_replay_cleanly() {
        assert_eq!(
            decode_collected_escape("\x1b"),
            vec![Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))]
        );
        assert_eq!(
            decode_collected_escape("\x1bb"),
            vec![Event::Key(KeyEvent::new(
                KeyCode::Char('b'),
                KeyModifiers::ALT
            ))]
        );
        assert_eq!(
            decode_collected_escape("\x1b[13;2u"),
            vec![Event::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::SHIFT
            ))]
        );
        assert!(decode_collected_escape("\x1b[?0u").is_empty());
    }
}

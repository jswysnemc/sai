use super::replay::{matching_prefix_with_control, ReplayRecord};
use super::WindowsPasteState;
use std::fs::OpenOptions;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Console::{
    PeekConsoleInputW, ReadConsoleInputW, INPUT_RECORD, KEY_EVENT, LEFT_ALT_PRESSED,
    LEFT_CTRL_PRESSED, RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED, SHIFT_PRESSED,
};

const BATCH_RECORDS: usize = 2048;
const MAX_BATCHES: usize = 16;
const DRAIN_BUDGET: Duration = Duration::from_millis(4);

/// 【终端】【Windows 粘贴】批量移除已经匹配的事件，保留第一个不同事件及其后内容
/// 参数: state 为已识别的粘贴回放；返回消费的事件数或控制台错误
/// 只能由输入循环在处理完一个匹配事件后调用，避免跨过 crossterm 已缓存的当前事件
pub(super) fn drain(state: &mut WindowsPasteState) -> io::Result<usize> {
    if state.replay.is_empty() && !state.control.has_pending_lf(Instant::now()) {
        return Ok(0);
    }
    // 1. 【终端】【Windows 粘贴】每批次复用句柄，避免每个字符反复打开控制台
    let console = OpenOptions::new().read(true).write(true).open("CONIN$")?;
    let handle = console.as_raw_handle().cast();
    let mut records: [INPUT_RECORD; BATCH_RECORDS] = unsafe { std::mem::zeroed() };
    let started = Instant::now();
    let mut total = 0;
    for _ in 0..MAX_BATCHES {
        let now = Instant::now();
        if (state.replay.is_empty() && !state.control.has_pending_lf(now))
            || started.elapsed() >= DRAIN_BUDGET
        {
            break;
        }
        let mut available = 0;
        // 2. 【终端】【Windows 粘贴】先查看再匹配，只读取确认属于当前正文的前缀
        if unsafe {
            PeekConsoleInputW(
                handle,
                records.as_mut_ptr(),
                BATCH_RECORDS as u32,
                &mut available,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let (matched, _) = matching_prefix_with_control(
            &state.replay[state.replay_offset..],
            &state.control,
            now,
            records[..available as usize].iter().map(replay_record),
        );
        if matched.records == 0 {
            break;
        }
        let mut consumed = 0;
        // 3. 【终端】【Windows 粘贴】输入循环独占读取，只消费已确认在队列中的前缀
        if unsafe {
            ReadConsoleInputW(
                handle,
                records.as_mut_ptr(),
                matched.records as u32,
                &mut consumed,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let (confirmed, control) = matching_prefix_with_control(
            &state.replay[state.replay_offset..],
            &state.control,
            now,
            records[..consumed as usize].iter().map(replay_record),
        );
        state.replay_offset += confirmed.bytes;
        state.control = control;
        total += consumed as usize;
        if state.replay_offset == state.replay.len() {
            state.replay.clear();
            state.replay_offset = 0;
        }
        state.finish_replay_if_done(now);
        if consumed as usize != matched.records || confirmed.records != consumed as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "console input changed during paste replay",
            ));
        }
    }
    Ok(total)
}

/// 【终端】【Windows 粘贴】提取可直接匹配的 UTF-16 单元，复杂按键交回原有事件解析
/// 参数: record 为控制台原始记录；返回可匹配文字、释放事件或必须停止的边界
fn replay_record(record: &INPUT_RECORD) -> ReplayRecord {
    if record.EventType != KEY_EVENT as u16 {
        return ReplayRecord::Boundary;
    }
    let key = unsafe { record.Event.KeyEvent };
    if key.bKeyDown == 0 {
        return ReplayRecord::Release;
    }
    // 1. 【终端】【ConPTY 换行】仅原始 LF 可携带 Control，普通快捷键仍停止批量消费
    let unit = unsafe { key.uChar.UnicodeChar };
    if unit == 0 && matches!(key.wVirtualKeyCode, 0x10..=0x12) {
        // 1. 【终端】【ConPTY 修饰键】这些记录不产生字符，实际快捷键仍由自身标志形成边界
        return ReplayRecord::Release;
    }
    if unit == 10
        && key.wRepeatCount == 1
        && key.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) == 0
        && matches!(key.wVirtualKeyCode, 0x0d | 0x4a)
    {
        return ReplayRecord::LineFeed;
    }
    if key.wRepeatCount != 1
        || key.dwControlKeyState
            & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED | LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)
            != 0
    {
        return ReplayRecord::Boundary;
    }
    match key.wVirtualKeyCode {
        0x0d | 0x09 if key.dwControlKeyState & SHIFT_PRESSED != 0 => ReplayRecord::Boundary,
        0x0d => ReplayRecord::Enter,
        0x09 => ReplayRecord::Unit(b'\t' as u16),
        0x08 | 0x1b | 0x10..=0x12 | 0x21..=0x2f | 0x70..=0x87 => ReplayRecord::Boundary,
        _ => {
            let unit = unsafe { key.uChar.UnicodeChar };
            if unit < 0x20 || unit == 0x7f {
                ReplayRecord::Boundary
            } else {
                ReplayRecord::Unit(unit)
            }
        }
    }
}

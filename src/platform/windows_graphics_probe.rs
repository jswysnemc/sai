//! Windows 图形查询的原生输入通道，完整保留用户输入记录与控制台模式。

use std::io;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Console::*;
use windows_sys::Win32::System::Threading::WaitForSingleObject;

pub(crate) struct ConsoleQuery {
    input: HANDLE,
    output: HANDLE,
    input_mode: u32,
    output_mode: u32,
}

impl ConsoleQuery {
    /// 【终端图片】【Windows 模式】验证控制台句柄，保存原有模式并临时启用 VT 输出。
    /// 参数: 无；返回: 模式恢复守卫，重定向或不支持 VT 时返回错误。
    pub(crate) fn open() -> io::Result<Self> {
        let input = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        let output = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        let mut input_mode = 0;
        let mut output_mode = 0;
        check(unsafe { GetConsoleMode(input, &mut input_mode) })?;
        check(unsafe { GetConsoleMode(output, &mut output_mode) })?;
        let guard = Self {
            input,
            output,
            input_mode,
            output_mode,
        };
        // 1. 【终端图片】【查询模式】保留已有标志；任一步失败均由守卫恢复原值
        check(unsafe {
            SetConsoleMode(
                input,
                input_mode & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT),
            )
        })?;
        check(unsafe {
            SetConsoleMode(
                output,
                output_mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING,
            )
        })?;
        Ok(guard)
    }

    /// 【终端图片】【原生读取】等待并读取一条原始记录，避免键盘解析器丢失零键码 ESC。
    /// 参数: timeout 为剩余等待时长；返回: 输入记录、超时空值或系统错误。
    pub(crate) fn read(&self, timeout: Duration) -> io::Result<Option<INPUT_RECORD>> {
        let millis = timeout.as_millis().clamp(1, u128::from(u32::MAX - 1)) as u32;
        match unsafe { WaitForSingleObject(self.input, millis) } {
            WAIT_OBJECT_0 => {
                let mut record = INPUT_RECORD::default();
                let mut read = 0;
                check(unsafe { ReadConsoleInputW(self.input, &mut record, 1, &mut read) })?;
                Ok((read == 1).then_some(record))
            }
            WAIT_TIMEOUT => Ok(None),
            _ => Err(io::Error::last_os_error()),
        }
    }

    /// 【终端图片】【原生归还】把读取过的普通输入按原始记录归还，保留按键、鼠标、代理对。
    /// 参数: records 为未被协议解析器消费的记录；返回: 回写结果。
    pub(crate) fn restore_input(&self, mut records: Vec<INPUT_RECORD>) -> io::Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        // 1. 【终端图片】【输入顺序】先接收当前队列中的后续事件，再将原始序列整体归还
        let mut count = 0;
        check(unsafe { GetNumberOfConsoleInputEvents(self.input, &mut count) })?;
        if count > 0 {
            let mut tail = vec![INPUT_RECORD::default(); count as usize];
            let mut read = 0;
            check(unsafe { ReadConsoleInputW(self.input, tail.as_mut_ptr(), count, &mut read) })?;
            records.extend_from_slice(&tail[..read as usize]);
        }
        let mut offset = 0;
        while offset < records.len() {
            let mut written = 0;
            let count = (records.len() - offset).min(u32::MAX as usize) as u32;
            check(unsafe {
                WriteConsoleInputW(self.input, records[offset..].as_ptr(), count, &mut written)
            })?;
            if written == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "console input was not restored",
                ));
            }
            offset += written as usize;
        }
        Ok(())
    }
}

impl Drop for ConsoleQuery {
    /// 【终端图片】【模式恢复】成功、失败和提前退出均准确恢复原始输入与输出标志。
    /// 参数: 无；返回: 无，恢复错误不覆盖查询错误。
    fn drop(&mut self) {
        unsafe {
            SetConsoleMode(self.input, self.input_mode);
            SetConsoleMode(self.output, self.output_mode);
        }
    }
}

/// 【终端图片】【Win32 返回值】把 BOOL 失败转换成系统错误。
/// 参数: success 为 API 返回值；返回: 成功或系统错误。
fn check(success: i32) -> io::Result<()> {
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

/// 【终端图片】【响应字符】读取原始 UnicodeChar，不依赖虚拟键码或键盘布局。
/// 参数: record 为原生输入记录；返回: 可匹配协议的 ASCII 片段，其余记录保持原样。
pub(crate) fn response_fragment(record: &INPUT_RECORD) -> Option<String> {
    if record.EventType != KEY_EVENT as u16 {
        return None;
    }
    let key = unsafe { record.Event.KeyEvent };
    if key.bKeyDown == 0 || key.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED) != 0 {
        return None;
    }
    let ch = char::from_u32(u32::from(unsafe { key.uChar.UnicodeChar }))?;
    if ch == '\x1b' || (ch.is_ascii() && !ch.is_control()) {
        // 1. 【终端图片】【ConPTY 兼容】部分宿主将 ESC 加字符表示为 Alt 记录，仍需还原协议边界
        let fragment = if ch != '\x1b'
            && key.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED) != 0
        {
            format!("\x1b{ch}")
        } else {
            ch.to_string()
        };
        Some(fragment.repeat(usize::from(key.wRepeatCount).max(1)))
    } else {
        None
    }
}

use std::fs::{File, OpenOptions};
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Console::*;

pub(super) struct ConsoleFixture {
    input: File,
    output: File,
    original: [HANDLE; 3],
    modes: (u32, u32),
}

impl ConsoleFixture {
    /// 【终端测试】【控制台隔离】保存标准句柄并分配测试专用控制台。
    /// 参数: 无；返回: 自动恢复句柄的夹具。
    pub(super) fn new() -> Self {
        let original = unsafe {
            [
                GetStdHandle(STD_INPUT_HANDLE),
                GetStdHandle(STD_OUTPUT_HANDLE),
                GetStdHandle(STD_ERROR_HANDLE),
            ]
        };
        unsafe { FreeConsole() };
        assert_ne!(unsafe { AllocConsole() }, 0);
        let input = OpenOptions::new()
            .read(true)
            .write(true)
            .open("CONIN$")
            .unwrap();
        let output = OpenOptions::new()
            .read(true)
            .write(true)
            .open("CONOUT$")
            .unwrap();
        unsafe {
            SetStdHandle(STD_INPUT_HANDLE, input.as_raw_handle() as _);
            SetStdHandle(STD_OUTPUT_HANDLE, output.as_raw_handle() as _);
            SetStdHandle(STD_ERROR_HANDLE, original[2]);
            FlushConsoleInputBuffer(input.as_raw_handle() as _);
        }
        // 1. 【终端测试】【非默认模式】不能用“恢复为 cooked”代替精确恢复用户原来的标志
        let modes = (
            ENABLE_LINE_INPUT | ENABLE_WINDOW_INPUT,
            ENABLE_PROCESSED_OUTPUT | ENABLE_WRAP_AT_EOL_OUTPUT,
        );
        assert_ne!(
            unsafe { SetConsoleMode(input.as_raw_handle() as _, modes.0) },
            0
        );
        assert_ne!(
            unsafe { SetConsoleMode(output.as_raw_handle() as _, modes.1) },
            0
        );
        Self {
            input,
            output,
            original,
            modes,
        }
    }

    /// 【终端测试】【输入句柄】读取测试控制台的输入句柄。
    /// 参数: 无；返回: 借用的 Win32 句柄。
    pub(super) fn input(&self) -> HANDLE {
        self.input.as_raw_handle() as _
    }

    /// 【终端测试】【查询模式】确认请求发送时已开启 VT 输出和无回显输入。
    /// 参数: 无；返回: 无。
    pub(super) fn assert_query_modes(&self) {
        let modes = self.current_modes();
        assert_ne!(modes.1 & ENABLE_VIRTUAL_TERMINAL_PROCESSING, 0);
        assert_eq!(
            modes.0 & (ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT),
            0
        );
    }

    /// 【终端测试】【模式恢复】查询前后的标志必须完全一致。
    /// 参数: 无；返回: 无。
    pub(super) fn assert_modes_restored(&self) {
        assert_eq!(self.current_modes(), self.modes);
    }

    /// 【终端测试】【模式读取】读取真实控制台的输入和输出模式。
    /// 参数: 无；返回: 模式二元组。
    fn current_modes(&self) -> (u32, u32) {
        let mut input = 0;
        let mut output = 0;
        assert_ne!(unsafe { GetConsoleMode(self.input(), &mut input) }, 0);
        assert_ne!(
            unsafe { GetConsoleMode(self.output.as_raw_handle() as _, &mut output) },
            0
        );
        (input, output)
    }

    /// 【终端测试】【输出重定向】将标准输出临时指向普通文件。
    /// 参数: file 为存活期间的目标文件；返回: 无。
    pub(super) fn redirect_output(&self, file: &File) {
        assert_ne!(
            unsafe { SetStdHandle(STD_OUTPUT_HANDLE, file.as_raw_handle() as _) },
            0
        );
    }

    /// 【终端测试】【输出归还】恢复测试控制台输出。
    /// 参数: 无；返回: 无。
    pub(super) fn restore_output(&self) {
        assert_ne!(
            unsafe { SetStdHandle(STD_OUTPUT_HANDLE, self.output.as_raw_handle() as _) },
            0
        );
    }

    /// 【终端测试】【记录快照】读取剩余原生记录，供输入完整性断言使用。
    /// 参数: 无；返回: 控制台当前记录列表。
    pub(super) fn remaining(&self) -> Vec<INPUT_RECORD> {
        let mut count = 0;
        assert_ne!(
            unsafe { GetNumberOfConsoleInputEvents(self.input(), &mut count) },
            0
        );
        if count == 0 {
            return Vec::new();
        }
        let mut records = vec![INPUT_RECORD::default(); count as usize];
        let mut read = 0;
        assert_ne!(
            unsafe { ReadConsoleInputW(self.input(), records.as_mut_ptr(), count, &mut read) },
            0
        );
        records.truncate(read as usize);
        records
    }
}

impl Drop for ConsoleFixture {
    /// 【终端测试】【夹具恢复】释放测试控制台并归还测试框架原有标准句柄。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        unsafe {
            FreeConsole();
            SetStdHandle(STD_INPUT_HANDLE, self.original[0]);
            SetStdHandle(STD_OUTPUT_HANDLE, self.original[1]);
            SetStdHandle(STD_ERROR_HANDLE, self.original[2]);
        }
    }
}

/// 【终端测试】【协议记录】按 UnicodeChar 构造零虚拟键码的控制台输入。
/// 参数: text 为文本；返回: 原始输入记录。
pub(super) fn text_records(text: &str) -> Vec<INPUT_RECORD> {
    text.encode_utf16()
        .map(|unit| INPUT_RECORD {
            EventType: KEY_EVENT as u16,
            Event: INPUT_RECORD_0 {
                KeyEvent: KEY_EVENT_RECORD {
                    bKeyDown: 1,
                    wRepeatCount: 1,
                    wVirtualKeyCode: 0,
                    wVirtualScanCode: 0,
                    uChar: KEY_EVENT_RECORD_0 { UnicodeChar: unit },
                    dwControlKeyState: 0,
                },
            },
        })
        .collect()
}

/// 【终端测试】【输入注入】把文本作为终端应答写入原生队列。
/// 参数: input 为控制台句柄，text 为文本；返回: 无。
pub(super) fn write_text(input: HANDLE, text: &str) {
    write_records(input, &text_records(text));
}

/// 【终端测试】【原始注入】不经过键盘转换地写入输入记录。
/// 参数: input 为控制台句柄，records 为原始记录；返回: 无。
pub(super) fn write_records(input: HANDLE, records: &[INPUT_RECORD]) {
    let mut written = 0;
    assert_ne!(
        unsafe { WriteConsoleInputW(input, records.as_ptr(), records.len() as u32, &mut written) },
        0
    );
    assert_eq!(written as usize, records.len());
}

/// 【终端测试】【键盘快照】保留全部键盘字段，忽略测试窗口产生的无关鼠标移动。
/// 参数: records 为原始记录；返回: 可以直接比较的键盘快照。
pub(super) fn key_snapshot(records: &[INPUT_RECORD]) -> Vec<(i32, u16, u16, u16, u16, u32)> {
    records
        .iter()
        .filter(|record| record.EventType == KEY_EVENT as u16)
        .map(|record| {
            let key = unsafe { record.Event.KeyEvent };
            (
                key.bKeyDown,
                key.wRepeatCount,
                key.wVirtualKeyCode,
                key.wVirtualScanCode,
                unsafe { key.uChar.UnicodeChar },
                key.dwControlKeyState,
            )
        })
        .collect()
}

//! Windows 控制台宿主与 VT 可视窗口的差异。
//!
//! crossterm 0.28 把 `WINDOW_BUFFER_SIZE_EVENT.dwSize` 再加 1 后当作
//! `Event::Resize`。`dwSize` 是屏幕缓冲区（含回滚），不是可见窗口。
//! 可见行列在 `GetConsoleScreenBufferInfo.srWindow` 里，`terminal::size()` 读的就是它。
//!
//! 另外，未设置 `DISABLE_NEWLINE_AUTO_RETURN` 时，写满一行会立刻换行并把画面顶上去；
//! 若父进程残留开启 `ENABLE_VIRTUAL_TERMINAL_INPUT`，ConPTY 会停止将方向键、Home/End/Shift+Tab
//! 等翻译为 Win32 输入记录，导致跨终端快捷键失效。

#[cfg(all(windows, not(test)))]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// `ENABLE_PROCESSED_OUTPUT`
#[cfg(any(windows, test))]
const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
/// `ENABLE_VIRTUAL_TERMINAL_PROCESSING`
#[cfg(any(windows, test))]
const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
/// `DISABLE_NEWLINE_AUTO_RETURN`
#[cfg(any(windows, test))]
const DISABLE_NEWLINE_AUTO_RETURN: u32 = 0x0008;
/// `ENABLE_VIRTUAL_TERMINAL_INPUT`
#[cfg(any(windows, test))]
const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x0200;

#[cfg(all(windows, not(test)))]
static OUTPUT_MODE_DEPTH: AtomicU32 = AtomicU32::new(0);
#[cfg(all(windows, not(test)))]
static ORIGINAL_OUTPUT_MODE: AtomicU32 = AtomicU32::new(0);
#[cfg(all(windows, not(test)))]
static OUTPUT_MODE_APPLIED: AtomicBool = AtomicBool::new(false);
#[cfg(all(windows, not(test)))]
static ORIGINAL_INPUT_MODE: AtomicU32 = AtomicU32::new(0);
#[cfg(all(windows, not(test)))]
static INPUT_MODE_APPLIED: AtomicBool = AtomicBool::new(false);

/// 选择本次 Resize 应采用的可见窗口尺寸。
///
/// 有控制台窗口矩形时以它为准，忽略事件里多出来的 1 和回滚缓冲高度。
/// 读不到矩形时保留事件报告，避免在没有控制台时改成 0。
///
/// 参数:
/// - `reported_cols`: 事件报告的列数
/// - `reported_rows`: 事件报告的行数
/// - `measured`: 可见窗口矩形；读不到时为 `None`
///
/// 返回:
/// - 至少为 1 的列数与行数
pub(crate) fn resolve_windows_resize(
    reported_cols: u16,
    reported_rows: u16,
    measured: Option<(u16, u16)>,
) -> (u16, u16) {
    if let Some((cols, rows)) = measured {
        if cols > 0 && rows > 0 {
            return (cols, rows);
        }
    }
    (reported_cols.max(1), reported_rows.max(1))
}

/// 把 Resize 事件换成当前平台可信的可见尺寸。
///
/// Windows 上读可见窗口；其他平台使用事件值。测试进程不查询控制台，
/// 避免套件改到运行测试的终端。
///
/// 参数:
/// - `cols`: 事件列数
/// - `rows`: 事件行数
///
/// 返回:
/// - 用于布局的列数与行数
pub(crate) fn viewport_resize(cols: u16, rows: u16) -> (u16, u16) {
    #[cfg(windows)]
    {
        if !cfg!(test) {
            let measured = crossterm::terminal::size()
                .ok()
                .filter(|(width, height)| *width > 0 && *height > 0);
            return resolve_windows_resize(cols, rows, measured);
        }
    }
    resolve_windows_resize(cols, rows, None)
}

/// 计算 TUI 需要的控制台输出模式。
///
/// 保留原有标志，并打开虚拟终端处理与“延迟换行”。
/// 延迟换行让写满最后一列的行为与 Unix 终端一致，不再立刻把后续内容顶到下一行。
///
/// 参数:
/// - `current`: 当前 `GetConsoleMode` 输出模式
///
/// 返回:
/// - 进入 TUI 后应设置的模式
#[cfg(any(windows, test))]
pub(crate) fn tui_output_mode(current: u32) -> u32 {
    current
        | ENABLE_PROCESSED_OUTPUT
        | ENABLE_VIRTUAL_TERMINAL_PROCESSING
        | DISABLE_NEWLINE_AUTO_RETURN
}

/// 【终端】【Windows 输入模式】计算 TUI 需要的控制台输入模式，清除残留的 VT 输入标志。
///
/// 保持 Win32 输入记录模式，使 ConPTY 正常翻译方向键/功能键，同时放行未识别的 Kitty CSI-u 序列。
///
/// 参数:
/// - `current`: 当前 `GetConsoleMode` 输入模式
///
/// 返回:
/// - 进入 TUI 后应设置的输入模式
#[cfg(any(windows, test))]
pub(crate) fn tui_input_mode(current: u32) -> u32 {
    current & !ENABLE_VIRTUAL_TERMINAL_INPUT
}

/// 进入 TUI 时修正 Windows 控制台输出与输入模式。嵌套调用只计深度。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn enter_tui_output_mode() {
    #[cfg(all(windows, not(test)))]
    enter_windows_modes();
}

/// 【终端】【Windows 输入保活】交互子控件归还终端后再次确保输入记录模式生效。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn ensure_tui_input_mode() {
    #[cfg(all(windows, not(test)))]
    ensure_windows_input_record_mode();
}

/// 离开一层 TUI 输入守卫。最后一层退出时恢复进入前的输出与输入模式。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn leave_tui_output_mode() {
    #[cfg(all(windows, not(test)))]
    leave_windows_modes();
}

/// panic 或紧急恢复时立刻还原输出与输入模式，不再等待守卫配对。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
pub(crate) fn force_restore_tui_output_mode() {
    #[cfg(all(windows, not(test)))]
    {
        OUTPUT_MODE_DEPTH.store(0, Ordering::SeqCst);
        restore_windows_modes();
    }
}

/// 读取标准输出的控制台模式。管道或非控制台句柄返回 `None`。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 句柄与当前模式
#[cfg(all(windows, not(test)))]
fn console_output_mode() -> Option<(windows_sys::Win32::Foundation::HANDLE, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Console::GetConsoleMode;

    let handle = std::io::stdout().as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    let mut mode = 0u32;
    if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
        return None;
    }
    Some((handle, mode))
}

/// 【终端】【Windows 输入查询】读取标准输入的控制台模式。管道或非控制台句柄返回 `None`。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 句柄与当前模式
#[cfg(all(windows, not(test)))]
fn console_input_mode() -> Option<(windows_sys::Win32::Foundation::HANDLE, u32)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Console::GetConsoleMode;

    let handle = std::io::stdin().as_raw_handle() as windows_sys::Win32::Foundation::HANDLE;
    let mut mode = 0u32;
    if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
        return None;
    }
    Some((handle, mode))
}

/// 第一层进入时写入 TUI 输出模式与输入记录模式。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[cfg(all(windows, not(test)))]
fn enter_windows_modes() {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    if OUTPUT_MODE_DEPTH.fetch_add(1, Ordering::SeqCst) != 0 {
        ensure_windows_input_record_mode();
        return;
    }
    // 1. 【终端】【输出模式】启用 VT 处理与延迟换行
    if let Some((handle, mode)) = console_output_mode() {
        ORIGINAL_OUTPUT_MODE.store(mode, Ordering::SeqCst);
        if unsafe { SetConsoleMode(handle, tui_output_mode(mode)) } != 0 {
            OUTPUT_MODE_APPLIED.store(true, Ordering::SeqCst);
        }
    }
    // 2. 【终端】【输入模式】清除残留 VT 输入标志并保存原始值以便退出恢复
    if let Some((handle, mode)) = console_input_mode() {
        ORIGINAL_INPUT_MODE.store(mode, Ordering::SeqCst);
        let next = tui_input_mode(mode);
        if next == mode || unsafe { SetConsoleMode(handle, next) } != 0 {
            INPUT_MODE_APPLIED.store(next != mode, Ordering::SeqCst);
        }
    }
}

/// 确保存储在标准输入句柄上的 VT 输入标志已关闭。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[cfg(all(windows, not(test)))]
fn ensure_windows_input_record_mode() {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    let Some((handle, mode)) = console_input_mode() else {
        return;
    };
    let next = tui_input_mode(mode);
    if next != mode {
        unsafe {
            SetConsoleMode(handle, next);
        }
    }
}

/// 最后一层离开时恢复输出与输入模式。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[cfg(all(windows, not(test)))]
fn leave_windows_modes() {
    // 1. 紧急恢复可能已经把深度清零，这里不能再减，否则会绕回
    let mut depth = OUTPUT_MODE_DEPTH.load(Ordering::SeqCst);
    loop {
        if depth == 0 {
            return;
        }
        match OUTPUT_MODE_DEPTH.compare_exchange(
            depth,
            depth - 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                if depth == 1 {
                    restore_windows_modes();
                }
                return;
            }
            Err(actual) => depth = actual,
        }
    }
}

/// 把输出与输入模式恢复成进入 TUI 之前的值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[cfg(all(windows, not(test)))]
fn restore_windows_modes() {
    use windows_sys::Win32::System::Console::SetConsoleMode;

    if OUTPUT_MODE_APPLIED.swap(false, Ordering::SeqCst) {
        if let Some((handle, _)) = console_output_mode() {
            unsafe {
                SetConsoleMode(handle, ORIGINAL_OUTPUT_MODE.load(Ordering::SeqCst));
            }
        }
    }
    if INPUT_MODE_APPLIED.swap(false, Ordering::SeqCst) {
        if let Some((handle, mode)) = console_input_mode() {
            let original_vt = ORIGINAL_INPUT_MODE.load(Ordering::SeqCst) & ENABLE_VIRTUAL_TERMINAL_INPUT;
            unsafe {
                SetConsoleMode(handle, (mode & !ENABLE_VIRTUAL_TERMINAL_INPUT) | original_vt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_windows_resize, tui_input_mode, tui_output_mode};

    /// 【终端】【Windows 尺寸】缓冲区高度和 crossterm 多加的 1 不能覆盖可见窗口。
    #[test]
    fn measured_viewport_wins_over_buffer_and_off_by_one() {
        assert_eq!(resolve_windows_resize(101, 31, Some((100, 30))), (100, 30));
        assert_eq!(
            resolve_windows_resize(121, 9001, Some((120, 40))),
            (120, 40)
        );
    }

    /// 【终端】【Windows 尺寸】没有控制台矩形时保留事件报告。
    #[test]
    fn missing_measurement_keeps_reported_size() {
        assert_eq!(resolve_windows_resize(80, 24, None), (80, 24));
        assert_eq!(resolve_windows_resize(0, 0, Some((0, 0))), (1, 1));
    }

    /// 【终端】【Windows 换行】输出模式必须打开延迟换行，且不清除原有标志。
    #[test]
    fn tui_output_mode_enables_deferred_wrap() {
        let current = 0x0002u32;
        let next = tui_output_mode(current);
        assert_eq!(next & current, current);
        assert_eq!(next & 0x0004, 0x0004);
        assert_eq!(next & 0x0008, 0x0008);
    }

    /// 【终端】【Windows 输入模式】必须清除 ENABLE_VIRTUAL_TERMINAL_INPUT 且保留其他标志。
    #[test]
    fn tui_input_mode_clears_virtual_terminal_input_flag() {
        let current = 0x0218u32;
        let next = tui_input_mode(current);
        assert_eq!(next & 0x0200, 0);
        assert_eq!(next, 0x0018u32);
    }
}

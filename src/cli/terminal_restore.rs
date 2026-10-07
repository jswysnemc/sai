use super::keyboard_enhancement::KeyboardEnhancementState;
use anyhow::Result;
use crossterm::cursor::Show;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use crossterm::terminal;
use std::io::{self, Write};

/// REPL 终端输入模式的 RAII 守卫。
///
/// 统一管理 raw mode、bracketed paste 与键盘增强协议的启停。
/// 正常路径调用 `finish` 显式恢复并拿到错误；错误或 panic 展开时
/// Drop 执行尽力恢复，保证用户 shell 不会停留在 raw mode。
pub(super) struct TerminalInputGuard {
    enhancement: KeyboardEnhancementState,
    finished: bool,
    // --- 新增：Windows 控制台输出模式守卫 ---
    output_mode: bool,
}

impl TerminalInputGuard {
    /// 启用 REPL 终端输入模式。
    ///
    /// 参数:
    /// - `stdout`: 终端输出
    /// - `show_cursor`: 是否同时强制显示光标；流式阶段由渲染器管理光标，传 false
    ///
    /// 返回:
    /// - 输入模式守卫；启用失败时已回滚 raw mode
    pub(super) fn enable(stdout: &mut io::Stdout, show_cursor: bool) -> Result<Self> {
        // 进入 raw mode 前装好终止信号监听，被 kill 时同样能恢复终端
        super::terminal_signals::install();
        terminal::enable_raw_mode()?;
        let mode_result = if show_cursor {
            execute!(stdout, Show, EnableBracketedPaste)
        } else {
            execute!(stdout, EnableBracketedPaste)
        };
        if let Err(err) = mode_result {
            let _ = terminal::disable_raw_mode();
            return Err(err.into());
        }
        self::enable_windows_output_mode();
        Ok(Self {
            enhancement: KeyboardEnhancementState::enable(stdout),
            finished: false,
            output_mode: true,
        })
    }

    /// 显式恢复终端输入模式。
    ///
    /// 幂等：重复调用只恢复一次。恢复动作全部执行完毕后再上报首个错误，
    /// 单步失败不会跳过其余恢复。
    ///
    /// 参数:
    /// - `stdout`: 终端输出
    ///
    /// 返回:
    /// - 恢复是否成功
    pub(super) fn finish(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.release_windows_output_mode();
        self.finished = true;
        let paste_result = execute!(stdout, DisableBracketedPaste);
        self.enhancement.disable(stdout);
        let raw_result = terminal::disable_raw_mode();
        paste_result?;
        raw_result?;
        Ok(())
    }

    // --- 新增：Windows 控制台输出模式 ---

    /// 退出本层输入守卫时恢复 Windows 控制台输出模式。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn release_windows_output_mode(&mut self) {
        if !self.output_mode {
            return;
        }
        self.output_mode = false;
        crate::platform::windows_console::leave_tui_output_mode();
    }
}

/// 进入 TUI 时打开 Windows 延迟换行。非 Windows 与测试进程为空操作。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
fn enable_windows_output_mode() {
    crate::platform::windows_console::enter_tui_output_mode();
}

impl Drop for TerminalInputGuard {
    /// 未显式恢复时（错误提前返回或 panic 展开）执行尽力恢复。
    ///
    /// 返回:
    /// - 无；恢复失败静默忽略，Drop 中不可再失败
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.release_windows_output_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, DisableBracketedPaste);
        self.enhancement.disable(&mut stdout);
        let _ = terminal::disable_raw_mode();
    }
}

/// 交互控件（权限、提问）归还终端后恢复流式阶段的输入模式。
///
/// 键盘增强协议是栈式的，由本轮的 TerminalInputGuard 统一管理，
/// 这里只恢复非栈式的 raw mode 与 bracketed paste，重复启用无副作用。
///
/// 返回:
/// - 恢复是否成功
pub(super) fn restore_stream_terminal_modes() -> Result<()> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnableBracketedPaste)?;
    Ok(())
}

/// 安装全局 panic 钩子，panic 时先尽力恢复终端再执行默认输出。
///
/// 没有这层兜底时，raw mode 或备用屏内 panic 会让用户 shell 不可用，
/// panic 消息本身也会被 raw mode 吞掉换行而难以阅读。
///
/// 返回:
/// - 无
pub(crate) fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        emergency_restore();
        default_hook(info);
    }));
}

/// 【终端】【恢复序列】返回把终端从 sai 的各种模式中恢复出来的完整序列。
///
/// 不追踪当前处于哪种模式，全部恢复序列无条件发出：多余的会被终端忽略，漏发才不可挽回。
/// 先在当前屏幕弹掉本进程压入的键盘增强层，再退出备用屏。
///
/// 返回:
/// - 恢复序列
pub(crate) fn restore_sequence() -> String {
    format!(
        "{}{}",
        super::alternate_screen::take_pop_all_sequence(),
        concat!(
            // 括号粘贴、鼠标捕获（按钮、拖动、任意移动、SGR 编码）
            "\x1b[?2004l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l",
            // 退出备用屏并显示光标
            "\x1b[?1049l\x1b[?25h",
            // 结束可能未闭合的同步更新并恢复自动换行
            "\x1b[?2026l\x1b[?7h",
        )
    )
}

/// 尽力恢复终端到可用状态（panic 钩子使用）。
///
/// 返回:
/// - 无
pub(crate) fn emergency_restore() {
    let mut stdout = io::stdout();
    let _ = stdout.write_all(restore_sequence().as_bytes());
    let _ = stdout.flush();
    crate::platform::windows_console::force_restore_tui_output_mode();
    // 关闭 raw mode，让后续输出恢复正常换行
    let _ = terminal::disable_raw_mode();
}

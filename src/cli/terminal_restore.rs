use super::keyboard_enhancement::KeyboardEnhancementState;
use anyhow::Result;
use crossterm::cursor::Show;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, PopKeyboardEnhancementFlags};
use crossterm::terminal::{self, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

/// REPL 终端输入模式的 RAII 守卫。
///
/// 统一管理 raw mode、bracketed paste 与键盘增强协议的启停。
/// 正常路径调用 `finish` 显式恢复并拿到错误；错误或 panic 展开时
/// Drop 执行尽力恢复，保证用户 shell 不会停留在 raw mode。
pub(super) struct TerminalInputGuard {
    enhancement: KeyboardEnhancementState,
    finished: bool,
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
        Ok(Self {
            enhancement: KeyboardEnhancementState::enable(stdout),
            finished: false,
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
        self.finished = true;
        let paste_result = execute!(stdout, DisableBracketedPaste);
        self.enhancement.disable(stdout);
        let raw_result = terminal::disable_raw_mode();
        paste_result?;
        raw_result?;
        Ok(())
    }
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

/// 尽力恢复终端到可用状态。
///
/// 不追踪当前处于哪种模式，全部恢复序列无条件发出：
/// 多余的恢复序列会被终端忽略，漏发才是不可挽回的。
///
/// 返回:
/// - 无
pub(crate) fn emergency_restore() {
    let mut stdout = io::stdout();
    // 1. 退出备用屏与键盘增强，恢复粘贴模式与光标
    let _ = queue!(
        stdout,
        PopKeyboardEnhancementFlags,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        Show
    );
    let _ = stdout.flush();
    // 2. 关闭 raw mode，让后续输出恢复正常换行
    let _ = terminal::disable_raw_mode();
}

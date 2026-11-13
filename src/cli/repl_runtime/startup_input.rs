//! 启动输入模式先于欢迎信息，正式输入或自动轮次接管同一个守卫。
use super::ReplRuntime;
use crate::cli::terminal_restore::TerminalInputGuard;
use anyhow::Result;

impl ReplRuntime {
    /// 【终端】【启动输入】无参数，返回启用结果，在首次可见输入框之前完成键盘探测
    pub(in crate::cli) fn enable_startup_input(&mut self) -> Result<()> {
        self.startup_input_guard = Some(TerminalInputGuard::enable(&mut std::io::stdout(), true)?);
        Ok(())
    }

    /// 【终端】【输入交接】参数为输出流与光标开关，返回启动守卫或新的输入守卫，避免重复探测和中途切回行输入
    pub(in crate::cli) fn acquire_input_guard(
        &mut self,
        stdout: &mut std::io::Stdout,
        show_cursor: bool,
    ) -> Result<TerminalInputGuard> {
        match self.startup_input_guard.take() {
            Some(guard) => Ok(guard),
            None => TerminalInputGuard::enable(stdout, show_cursor),
        }
    }
}

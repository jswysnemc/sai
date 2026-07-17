use anyhow::Result;
use portable_pty::CommandBuilder;
use std::io::Write;

const CONPTY_CURSOR_POSITION_RESPONSE: &[u8] = b"\x1b[1;1R";

/// 根据终端配置构造 PTY 启动命令。
///
/// 参数:
/// - `configured_shell`: 用户配置的 Shell 可执行文件路径或名称
///
/// 返回:
/// - PTY 命令构造器
pub(super) fn terminal_command(configured_shell: &str) -> CommandBuilder {
    match crate::platform::shell::terminal_shell_invocation(configured_shell) {
        Some(shell) => {
            let mut command = CommandBuilder::new(&shell.program);
            command.args(&shell.args);
            command
        }
        None => CommandBuilder::new_default_prog(),
    }
}

/// 初始化 PTY 输入通道。
///
/// 参数:
/// - `writer`: PTY 输入写入端
/// - `windows_conpty`: 是否使用 Windows ConPTY
///
/// 返回:
/// - 初始化结果
pub(super) fn initialize_pty_writer(writer: &mut dyn Write, windows_conpty: bool) -> Result<()> {
    if windows_conpty {
        // 【Windows终端】【初始化ConPTY】1. 回应光标位置查询，避免 Shell 启动阶段持续等待
        writer.write_all(CONPTY_CURSOR_POSITION_RESPONSE)?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 Windows ConPTY 初始化会写入光标位置响应。
    #[test]
    fn writes_windows_conpty_cursor_response() {
        let mut output = Vec::new();

        initialize_pty_writer(&mut output, true).expect("ConPTY 初始化应成功");

        assert_eq!(output, b"\x1b[1;1R");
    }

    /// 验证原生 PTY 初始化不会写入额外输入。
    #[test]
    fn leaves_native_pty_input_empty() {
        let mut output = Vec::new();

        initialize_pty_writer(&mut output, false).expect("原生 PTY 初始化应成功");

        assert!(output.is_empty());
    }

    /// 验证 Unix 空配置会使用 portable-pty 的用户登录 Shell。
    #[cfg(not(windows))]
    #[test]
    fn empty_shell_uses_portable_pty_default_program() {
        let command = terminal_command("");

        assert!(command.is_default_prog());
    }

    /// 验证用户配置的 Shell 会成为 PTY 启动程序。
    #[test]
    fn configured_shell_uses_requested_program() {
        let command = terminal_command("custom-shell");

        assert_eq!(
            command.get_argv(),
            &vec![std::ffi::OsString::from("custom-shell")]
        );
    }
}

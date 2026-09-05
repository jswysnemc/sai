use super::*;

/// 清理输入区并恢复终端状态，干净结束本次 REPL 输入循环。
///
/// 参数:
/// - `stdout`: 终端输出句柄
/// - `input_row`: 输入区起始行
/// - `rendered_rows`: 输入区已经渲染的行数
/// - `runtime`: REPL 终端运行期
/// - `terminal_guard`: 终端输入模式守卫
///
/// 返回:
/// - 清理与终端恢复是否成功
pub(super) fn finish_repl_input(
    stdout: &mut io::Stdout,
    input_row: u16,
    rendered_rows: u16,
    runtime: &mut ReplRuntime,
    terminal_guard: &mut crate::cli::terminal_restore::TerminalInputGuard,
) -> Result<()> {
    // 1. 清除 composer 已经绘制的全部终端行
    clear_repl_input(stdout, input_row, rendered_rows)?;
    // 2. 释放 composer 占用空间，使 transcript 尾部保持完整
    runtime.end_composer()?;
    // 3. 恢复 raw mode、粘贴模式与键盘增强协议
    terminal_guard.finish(stdout)
}

use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use super::repl_runtime::ReplRuntime;

const OUTPUT_LIMIT: usize = 20_000;
/// 等待子进程的轮询间隔：既是界面刷新节奏，也是状态行耗时的更新粒度。
const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// REPL 本地 Shell 命令的执行结果。
pub(super) struct ReplShellResult {
    pub(super) command: String,
    pub(super) output: String,
    pub(super) exit_code: Option<i32>,
}

/// 执行 `!` 命令并在等待期间持续刷新界面。
///
/// 直接 `.output()` 会把整个 REPL 主循环堵住：期间不重绘、不响应 resize，
/// 屏幕停在上一帧，用户以为程序卡死（尤其是 `!cargo build` 这类长命令）。
/// 这里改为轮询等待，每 100ms 驱动一次 live 刷新，让工作状态行的耗时持续走动。
///
/// 注意不消费终端输入事件：按键仍留给后续的输入循环，
/// 否则会吞掉用户在命令执行期间敲入的字符。
///
/// 参数:
/// - `command`: 不含 `!` 的 Shell 命令正文
/// - `runtime`: REPL 终端运行期
///
/// 返回:
/// - 命令、合并后输出与退出码
pub(super) async fn execute_repl_shell_live(
    command: &str,
    runtime: &mut ReplRuntime,
) -> Result<ReplShellResult> {
    let command = command.trim();
    if command.is_empty() {
        bail!(
            "{}",
            t("enter a Shell command after !", "请在 ! 后输入 Shell 命令")
        )
    }
    let invocation = crate::platform::shell::command_invocation(command);
    let cwd = crate::runtime_cwd::current_dir()?;
    let mut child = Command::new(&invocation.program)
        .args(&invocation.args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // 本函数返回或异步任务被取消时都要收掉子进程，不留孤儿
        .kill_on_drop(true)
        .spawn()
        .with_context(|| {
            format!(
                "{}: {}",
                t("Shell command failed", "Shell 命令执行失败"),
                invocation.program.to_string_lossy()
            )
        })?;
    // 管道必须在等待期间并发读空：子进程写满管道缓冲区会阻塞在 write 上，
    // 而父进程阻塞在 wait() 上，二者互相等待直接死锁
    let stdout_task = tokio::spawn(read_pipe_to_vec(child.stdout.take()));
    let stderr_task = tokio::spawn(read_pipe_to_vec(child.stderr.take()));

    runtime.begin_shell_status()?;
    let wait_result = loop {
        tokio::select! {
            status = child.wait() => break status,
            _ = tokio::time::sleep(WAIT_POLL_INTERVAL) => {
                runtime.tick_live()?;
            }
        }
    };
    runtime.end_shell_status()?;

    let status = wait_result.with_context(|| {
        format!(
            "{}: {}",
            t("Shell command failed", "Shell 命令执行失败"),
            invocation.program.to_string_lossy()
        )
    })?;
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();
    let mut output = crate::platform::output_encoding::decode_output(&stdout);
    let stderr = crate::platform::output_encoding::decode_output(&stderr);
    Ok(ReplShellResult {
        command: command.to_string(),
        output: truncate_output(&merge_output(&mut output, &stderr)),
        exit_code: status.code(),
    })
}

/// 把一个异步读句柄完整读到内存。
///
/// 参数:
/// - `reader`: 可选读句柄，为空时返回空缓冲
///
/// 返回:
/// - 读到的全部字节
async fn read_pipe_to_vec<R: AsyncRead + Unpin>(mut reader: Option<R>) -> Vec<u8> {
    let Some(reader) = reader.as_mut() else {
        return Vec::new();
    };
    let mut buffer = Vec::new();
    let _ = reader.read_to_end(&mut buffer).await;
    buffer
}

/// 合并标准输出与标准错误，保证两者之间换行完整。
///
/// 参数:
/// - `output`: 标准输出
/// - `stderr`: 标准错误
///
/// 返回:
/// - 合并后的文本
fn merge_output(output: &mut String, stderr: &str) -> String {
    if !stderr.is_empty() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(stderr);
    }
    output.clone()
}

/// 限制 Shell 输出进入 transcript 的字符数。
///
/// 参数:
/// - `output`: 原始标准输出与标准错误
///
/// 返回:
/// - 限制后的输出
fn truncate_output(output: &str) -> String {
    if output.chars().count() <= OUTPUT_LIMIT {
        return output.to_string();
    }
    let mut truncated = output.chars().take(OUTPUT_LIMIT).collect::<String>();
    truncated.push_str(t("\n[Shell output truncated]", "\n[Shell 输出已截断]"));
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

    fn test_options() -> crate::render::transcript::TranscriptRenderOptions {
        crate::render::transcript::TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Summary,
        }
    }

    #[tokio::test]
    async fn executes_shell_command_and_captures_output() {
        #[cfg(windows)]
        let command = "[Console]::Write('shell-test')";
        #[cfg(not(windows))]
        let command = "printf shell-test";
        let mut runtime = ReplRuntime::new(100, test_options());
        let result = execute_repl_shell_live(command, &mut runtime).await.unwrap();

        assert_eq!(result.command, command);
        assert_eq!(result.output, "shell-test");
        assert_eq!(result.exit_code, Some(0));
    }

    /// 等待期间会驱动 live 刷新：长命令不应让界面停在上一帧。
    #[tokio::test]
    async fn long_running_command_still_flushes_the_ui() {
        #[cfg(windows)]
        let command = "Start-Sleep -Milliseconds 350; [Console]::Write('done')";
        #[cfg(not(windows))]
        let command = "sleep 0.35; printf done";
        let mut runtime = ReplRuntime::new(100, test_options());

        let result = execute_repl_shell_live(command, &mut runtime).await.unwrap();

        assert_eq!(result.output, "done");
        assert_eq!(result.exit_code, Some(0));
        // 结束后工作状态必须清掉，否则状态行会一直挂在 transcript 上
        assert!(!runtime.transcript_has_work_status());
    }
}

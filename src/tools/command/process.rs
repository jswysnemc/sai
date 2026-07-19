use super::progress::{CommandOutputBatch, CommandOutputStream};
use crate::tools::ToolProgress;
use anyhow::{bail, Result};
use std::io::ErrorKind;
use std::process::{Output, Stdio};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{Instant, MissedTickBehavior};

const MAX_CAPTURED_OUTPUT_BYTES: usize = 80_004;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// 用 shell 执行短命令并在超时时清理进程。
///
/// 参数:
/// - `command`: shell 命令文本
/// - `timeout_seconds`: 超时时间，单位秒
/// - `configured_shell`: 配置指定的 shell，空值表示使用用户环境
///
/// 返回:
/// - 命令输出
pub(crate) async fn run_shell_command(
    command: &str,
    timeout_seconds: u64,
    configured_shell: &str,
    sandboxed: bool,
) -> Result<Output> {
    run_shell_command_with_optional_progress(
        command,
        timeout_seconds,
        configured_shell,
        sandboxed,
        None,
    )
    .await
}

/// 用 shell 执行短命令并实时报告 stdout 和 stderr。
///
/// 参数:
/// - `command`: shell 命令文本
/// - `timeout_seconds`: 超时时间，单位秒
/// - `configured_shell`: 配置指定的 shell，空值表示使用用户环境
/// - `sandboxed`: 是否使用只读沙盒
/// - `progress`: 工具进度通道
///
/// 返回:
/// - 命令输出
pub(crate) async fn run_shell_command_with_progress(
    command: &str,
    timeout_seconds: u64,
    configured_shell: &str,
    sandboxed: bool,
    progress: ToolProgress,
) -> Result<Output> {
    run_shell_command_with_optional_progress(
        command,
        timeout_seconds,
        configured_shell,
        sandboxed,
        Some(progress),
    )
    .await
}

/// 按可选进度通道执行 shell 命令。
///
/// 参数:
/// - `command`: shell 命令文本
/// - `timeout_seconds`: 超时时间，单位秒
/// - `configured_shell`: 配置指定的 shell
/// - `sandboxed`: 是否启用只读沙盒
/// - `progress`: 可选工具进度通道
///
/// 返回:
/// - 命令输出
async fn run_shell_command_with_optional_progress(
    command: &str,
    timeout_seconds: u64,
    configured_shell: &str,
    sandboxed: bool,
    progress: Option<ToolProgress>,
) -> Result<Output> {
    let duration = Duration::from_secs(timeout_seconds.max(1));
    let mut missing = Vec::new();
    for (program, mut shell) in shell_commands(command, configured_shell, sandboxed)? {
        match run_command_with_timeout(&mut shell, duration, progress.clone()).await {
            Ok(output) => return Ok(output),
            Err(CommandRunError::NotFound) => missing.push(program),
            Err(CommandRunError::Timeout) => {
                bail!("shell command timed out after {timeout_seconds}s")
            }
            Err(CommandRunError::Other(err)) => return Err(err),
        }
    }
    bail!("no supported shell found; tried {}", missing.join(", "))
}

/// 启动后台 shell 命令。
///
/// 参数:
/// - `command`: shell 命令文本
/// - `cwd`: 工作目录
/// - `configured_shell`: 配置指定的 shell，空值表示使用用户环境
/// - `stdout`: stdout 重定向
/// - `stderr`: stderr 重定向
///
/// 返回:
/// - 启动后的进程信息
pub(crate) fn spawn_background_shell(
    command: &str,
    cwd: &std::path::Path,
    configured_shell: &str,
    stdout: std::fs::File,
    stderr: std::fs::File,
) -> Result<BackgroundProcess> {
    let mut shell = shell_command(command, configured_shell);
    configure_process_group(&mut shell);
    let child = shell
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .kill_on_drop(false)
        .spawn()?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow::anyhow!("background process id is unavailable"))?;
    drop(child);
    Ok(BackgroundProcess {
        pid,
        pgid: process_group_id(pid),
    })
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BackgroundProcess {
    pub(crate) pid: u32,
    pub(crate) pgid: Option<i32>,
}

enum CommandRunError {
    NotFound,
    Timeout,
    Other(anyhow::Error),
}

/// 执行命令并在超时时终止进程组。
///
/// 参数:
/// - `command`: 已配置的命令
/// - `duration`: 超时时间
///
/// 返回:
/// - 命令输出
async fn run_command_with_timeout(
    command: &mut Command,
    duration: Duration,
    progress: Option<ToolProgress>,
) -> std::result::Result<Output, CommandRunError> {
    configure_process_group(command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|err| {
        if err.kind() == ErrorKind::NotFound {
            CommandRunError::NotFound
        } else {
            CommandRunError::Other(err.into())
        }
    })?;
    let pid = child.id();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_progress = progress.clone();
    let stderr_progress = progress;
    let stdout_task = tokio::spawn(async move {
        read_pipe(&mut stdout, stdout_progress, CommandOutputStream::Stdout).await
    });
    let stderr_task = tokio::spawn(async move {
        read_pipe(&mut stderr, stderr_progress, CommandOutputStream::Stderr).await
    });
    let status = match tokio::time::timeout(duration, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(err)) => return Err(CommandRunError::Other(err.into())),
        Err(_) => {
            if let Some(pid) = pid {
                terminate_process(pid, process_group_id(pid), true).await;
            } else {
                let _ = child.kill().await;
            }
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(CommandRunError::Timeout);
        }
    };
    let stdout = stdout_task
        .await
        .map_err(|err| CommandRunError::Other(err.into()))?
        .map_err(|err| CommandRunError::Other(err.into()))?;
    let stderr = stderr_task
        .await
        .map_err(|err| CommandRunError::Other(err.into()))?
        .map_err(|err| CommandRunError::Other(err.into()))?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// 读取子进程管道内容。
///
/// 参数:
/// - `pipe`: 可选管道
/// - `progress`: 可选工具进度通道
/// - `stream`: 输出流类型
///
/// 返回:
/// - 读取到的字节
async fn read_pipe<T>(
    pipe: &mut Option<T>,
    progress: Option<ToolProgress>,
    stream: CommandOutputStream,
) -> std::io::Result<Vec<u8>>
where
    T: tokio::io::AsyncRead + Unpin,
{
    let mut buffer = Vec::new();
    if let Some(pipe) = pipe {
        let mut chunk = [0u8; 4_096];
        let mut progress_batch = CommandOutputBatch::default();
        let mut progress_tick =
            tokio::time::interval_at(Instant::now() + PROGRESS_INTERVAL, PROGRESS_INTERVAL);
        progress_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                read = pipe.read(&mut chunk) => {
                    let read = read?;
                    if read == 0 {
                        break;
                    }
                    append_captured_output(&mut buffer, &chunk[..read]);
                    if progress.is_some() {
                        progress_batch.append(&chunk[..read]);
                    }
                }
                _ = progress_tick.tick(), if !progress_batch.is_empty() => {
                    report_command_output(&progress, stream, &mut progress_batch);
                }
            }
        }
        report_command_output(&progress, stream, &mut progress_batch);
    }
    Ok(buffer)
}

/// 上报并清空一批命令输出。
///
/// 参数:
/// - `progress`: 可选工具进度通道
/// - `stream`: 输出流类型
/// - `batch`: 待发送输出批次
///
/// 返回:
/// - 无
fn report_command_output(
    progress: &Option<ToolProgress>,
    stream: CommandOutputStream,
    batch: &mut CommandOutputBatch,
) {
    if let Some(progress) = progress {
        if let Some(message) = batch.take_message(stream) {
            progress.report(message);
        }
    }
}

/// 将输出追加到返回结果的有界缓冲。
///
/// 参数:
/// - `buffer`: 已捕获的输出
/// - `chunk`: 新输出片段
///
/// 返回:
/// - 无
fn append_captured_output(buffer: &mut Vec<u8>, chunk: &[u8]) {
    let remaining = MAX_CAPTURED_OUTPUT_BYTES.saturating_sub(buffer.len());
    buffer.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
}

#[cfg(test)]
mod output_capture_tests {
    use super::*;

    #[test]
    fn captured_output_does_not_exceed_memory_limit() {
        let mut buffer = vec![b'a'; MAX_CAPTURED_OUTPUT_BYTES - 2];
        append_captured_output(&mut buffer, b"bcdef");

        assert_eq!(buffer.len(), MAX_CAPTURED_OUTPUT_BYTES);
        assert!(buffer.ends_with(b"bc"));
    }

    #[tokio::test]
    async fn timeout_flushes_pending_progress_output() {
        #[cfg(windows)]
        let command = "Write-Output before-timeout; Start-Sleep -Seconds 2";
        #[cfg(not(windows))]
        let command = "printf before-timeout; sleep 2";
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        let error =
            run_shell_command_with_progress(command, 1, "", false, ToolProgress::new(sender))
                .await
                .unwrap_err();
        let output = std::iter::from_fn(|| receiver.try_recv().ok())
            .filter_map(|message| super::super::progress::decode_command_output(&message))
            .flat_map(|chunk| chunk.bytes)
            .collect::<Vec<_>>();

        assert!(error.to_string().contains("timed out"));
        assert!(String::from_utf8_lossy(&output).contains("before-timeout"));
    }
}

#[cfg(windows)]
fn shell_commands(
    command: &str,
    configured_shell: &str,
    sandboxed: bool,
) -> Result<Vec<(String, Command)>> {
    if sandboxed {
        bail!("audited sandbox mode is not supported on Windows")
    }
    if let Some(shell) = configured_shell_path(configured_shell) {
        return Ok(vec![shell_command_entry(command, &shell)]);
    }
    let mut pwsh = Command::new("pwsh");
    pwsh.arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(command);

    let mut powershell = Command::new("powershell");
    powershell
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(command);

    let mut cmd = Command::new("cmd");
    cmd.arg("/S").arg("/C").arg(command);

    Ok(vec![
        ("pwsh".to_string(), inherit_env(pwsh)),
        ("powershell".to_string(), inherit_env(powershell)),
        ("cmd".to_string(), inherit_env(cmd)),
    ])
}

#[cfg(target_os = "linux")]
fn shell_commands(
    command: &str,
    configured_shell: &str,
    sandboxed: bool,
) -> Result<Vec<(String, Command)>> {
    let shell = configured_shell_path(configured_shell)
        .or_else(|| std::env::var("SHELL").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "sh".to_string());
    if sandboxed {
        return Ok(vec![(
            "bwrap".to_string(),
            sandboxed_shell_command(command, &shell)?,
        )]);
    }
    Ok(vec![shell_command_entry(command, &shell)])
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn shell_commands(
    command: &str,
    configured_shell: &str,
    sandboxed: bool,
) -> Result<Vec<(String, Command)>> {
    if sandboxed {
        bail!("audited sandbox mode is only supported on Linux")
    }
    let shell = configured_shell_path(configured_shell)
        .or_else(|| std::env::var("SHELL").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "sh".to_string());
    Ok(vec![shell_command_entry(command, &shell)])
}

/// 构造 Linux 工作区写入沙盒命令。
///
/// 参数:
/// - `command`: Shell 命令文本
/// - `shell`: 沙盒内使用的 Shell
///
/// 返回:
/// - bubblewrap 命令
#[cfg(target_os = "linux")]
fn sandboxed_shell_command(command: &str, shell: &str) -> Result<Command> {
    let workspace = crate::runtime_cwd::current_dir()?;
    let mut process = Command::new("bwrap");
    process
        .arg("--die-with-parent")
        .arg("--new-session")
        .arg("--unshare-net")
        .arg("--ro-bind")
        .arg("/")
        .arg("/")
        .arg("--bind")
        .arg(&workspace)
        .arg(&workspace)
        .arg("--proc")
        .arg("/proc")
        .arg("--dev")
        .arg("/dev")
        .arg("--chdir")
        .arg(&workspace)
        .arg("--")
        .arg(shell)
        .arg("-lc")
        .arg(command);
    Ok(inherit_env(process))
}

#[cfg(all(test, target_os = "linux"))]
mod sandbox_tests {
    use super::*;

    /// 验证审计沙盒只允许写入当前工作区。
    #[tokio::test]
    async fn audited_sandbox_blocks_parent_directory_writes() {
        if std::process::Command::new("bwrap")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let output = crate::runtime_cwd::scope(workspace.clone(), async {
            run_shell_command("touch allowed && touch ../blocked", 10, "sh", true)
                .await
                .unwrap()
        })
        .await;
        assert!(!output.status.success());
        assert!(workspace.join("allowed").exists());
        assert!(!root.path().join("blocked").exists());
    }
}

#[cfg(windows)]
fn shell_command(command: &str, configured_shell: &str) -> Command {
    let shell = configured_shell_path(configured_shell).unwrap_or_else(|| "cmd".to_string());
    shell_command_entry(command, &shell).1
}

#[cfg(not(windows))]
fn shell_command(command: &str, configured_shell: &str) -> Command {
    let shell = configured_shell_path(configured_shell)
        .or_else(|| std::env::var("SHELL").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "sh".to_string());
    shell_command_entry(command, &shell).1
}

/// 解析显式配置的 shell。
///
/// 参数:
/// - `configured_shell`: TUI 或配置文件中的 shell 路径
///
/// 返回:
/// - 非空 shell 路径
fn configured_shell_path(configured_shell: &str) -> Option<String> {
    let value = configured_shell.trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// 构造 shell 命令入口。
///
/// 参数:
/// - `command`: shell 命令文本
/// - `shell`: shell 程序路径或名称
///
/// 返回:
/// - 展示名和已配置命令
fn shell_command_entry(command: &str, shell: &str) -> (String, Command) {
    let program = shell_display_name(shell);
    let mut shell_command = Command::new(shell);
    #[cfg(windows)]
    {
        let lower = program.to_ascii_lowercase();
        if lower == "cmd" || lower == "cmd.exe" {
            shell_command.arg("/S").arg("/C").arg(command);
        } else {
            shell_command
                .arg("-NoLogo")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-Command")
                .arg(command);
        }
    }
    #[cfg(not(windows))]
    {
        shell_command.arg("-lc").arg(command);
    }
    (program, inherit_env(shell_command))
}

/// 返回用于错误提示的 shell 名称。
///
/// 参数:
/// - `shell`: shell 路径或名称
///
/// 返回:
/// - shell 文件名或原始名称
fn shell_display_name(shell: &str) -> String {
    std::path::Path::new(shell)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| shell.to_string())
}

/// 显式继承当前进程环境变量。
///
/// 参数:
/// - `command`: 待配置命令
///
/// 返回:
/// - 已继承环境变量的命令
fn inherit_env(mut command: Command) -> Command {
    command.envs(std::env::vars());
    command
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn process_group_id(pid: u32) -> Option<i32> {
    Some(pid as i32)
}

#[cfg(not(unix))]
fn process_group_id(_pid: u32) -> Option<i32> {
    None
}

/// 判断进程是否仍存在。
///
/// 参数:
/// - `pid`: 进程 ID
///
/// 返回:
/// - 是否存在
pub(crate) fn process_exists(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

/// 终止进程或进程组。
///
/// 参数:
/// - `pid`: 进程 ID
/// - `pgid`: 进程组 ID
/// - `force`: 是否直接强杀
pub(crate) async fn terminate_process(pid: u32, pgid: Option<i32>, force: bool) {
    #[cfg(unix)]
    {
        let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
        let target = pgid.map(|pgid| -pgid).unwrap_or(pid as i32);
        unsafe {
            libc::kill(target, signal);
        }
    }
    #[cfg(windows)]
    {
        let mut command = tokio::process::Command::new("taskkill");
        command.arg("/PID").arg(pid.to_string()).arg("/T");
        if force {
            command.arg("/F");
        }
        let _ = command.output().await;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        let _ = pgid;
        let _ = force;
    }
}

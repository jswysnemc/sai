use anyhow::{bail, Result};
use std::io::ErrorKind;
use std::process::{Output, Stdio};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

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
    let duration = Duration::from_secs(timeout_seconds.max(1));
    let mut missing = Vec::new();
    for (program, mut shell) in shell_commands(command, configured_shell, sandboxed)? {
        match run_command_with_timeout(&mut shell, duration).await {
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
    let stdout_task = tokio::spawn(async move { read_pipe(&mut stdout).await });
    let stderr_task = tokio::spawn(async move { read_pipe(&mut stderr).await });
    let status = match tokio::time::timeout(duration, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(err)) => return Err(CommandRunError::Other(err.into())),
        Err(_) => {
            if let Some(pid) = pid {
                terminate_process(pid, process_group_id(pid), true).await;
            } else {
                let _ = child.kill().await;
            }
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
///
/// 返回:
/// - 读取到的字节
async fn read_pipe<T>(pipe: &mut Option<T>) -> std::io::Result<Vec<u8>>
where
    T: tokio::io::AsyncRead + Unpin,
{
    let mut buffer = Vec::new();
    if let Some(pipe) = pipe {
        pipe.read_to_end(&mut buffer).await?;
    }
    Ok(buffer)
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

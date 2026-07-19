use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::timeout;

const WEB_START_TIMEOUT: Duration = Duration::from_secs(10);
const WEB_URL_PREFIX: &str = "Sai Web: ";

/// 为指定工作区启动独立 Sai Web 进程。
///
/// 参数:
/// - `workspace`: 新窗口的工作区目录
///
/// 返回:
/// - 新服务包含一次性令牌的访问地址
pub(crate) async fn open_workspace_window(workspace: &Path) -> Result<String> {
    let workspace = canonical_workspace(workspace)?;
    let executable = std::env::current_exe().context("failed to locate Sai executable")?;
    let command = launch_command(&executable, &workspace);
    let mut child = tokio::process::Command::from(command)
        .spawn()
        .context("failed to start Sai Web workspace process")?;
    let stdout = child
        .stdout
        .take()
        .context("Sai Web workspace process stdout is unavailable")?;
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();

    // 1. 等待子进程完成端口绑定并输出带令牌地址
    let read = match timeout(WEB_START_TIMEOUT, reader.read_line(&mut line)).await {
        Ok(result) => result.context("failed to read Sai Web workspace process output")?,
        Err(_) => {
            terminate_child(&mut child).await;
            bail!("Sai Web workspace process did not start within 10 seconds")
        }
    };
    if read == 0 {
        terminate_child(&mut child).await;
        bail!("Sai Web workspace process exited before reporting its address");
    }
    let url = line
        .trim()
        .strip_prefix(WEB_URL_PREFIX)
        .filter(|value| value.starts_with("http://"))
        .context("Sai Web workspace process returned an invalid address")?
        .to_string();

    // 2. 后台持有子进程句柄，确保进程退出后由父进程回收
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(url)
}

/// 构造独立 Web 进程命令。
///
/// 参数:
/// - `executable`: 当前 Sai 可执行文件
/// - `workspace`: 新进程初始工作区
///
/// 返回:
/// - 已配置随机端口、关闭自动打开和标准流的命令
fn launch_command(executable: &Path, workspace: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("web")
        .arg("--port")
        .arg("0")
        .arg("--no-open")
        .arg("--workspace")
        .arg(workspace)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_detached_process(&mut command);
    command
}

/// 配置 Unix 子进程使用独立进程组。
///
/// 参数:
/// - `command`: 待启动命令
///
/// 返回:
/// - 无
#[cfg(unix)]
fn configure_detached_process(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// 配置 Windows 子进程脱离当前控制台并使用独立进程组。
///
/// 参数:
/// - `command`: 待启动命令
///
/// 返回:
/// - 无
#[cfg(windows)]
fn configure_detached_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}

/// 终止启动失败的子进程并完成回收。
///
/// 参数:
/// - `child`: 待终止子进程
///
/// 返回:
/// - 无
async fn terminate_child(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// 规范化新窗口工作区目录。
///
/// 参数:
/// - `workspace`: 待打开目录
///
/// 返回:
/// - 平台兼容的规范目录路径
fn canonical_workspace(workspace: &Path) -> Result<PathBuf> {
    let canonical = crate::platform::windows_path::canonicalize(workspace)
        .with_context(|| format!("workspace does not exist: {}", workspace.display()))?;
    if !canonical.is_dir() {
        bail!("workspace is not a directory: {}", canonical.display());
    }
    Ok(crate::platform::windows_path::simplified(&canonical))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证新窗口命令使用随机端口和独立工作区参数。
    #[test]
    fn builds_workspace_web_command() {
        let command = launch_command(Path::new("sai-test"), Path::new("workspace"));
        let args = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            vec![
                "web",
                "--port",
                "0",
                "--no-open",
                "--workspace",
                "workspace"
            ]
        );
    }
}

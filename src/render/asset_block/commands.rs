use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// 执行外部图片渲染命令。
///
/// 参数:
/// - `command`: 待执行命令
/// - `name`: 命令名称
///
/// 返回:
/// - 命令是否成功
pub(super) fn run_command(mut command: Command, name: &str) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("failed to run {name}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let message = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!("{name} exited with status {}: {message}", output.status);
    }
    Ok(())
}

/// 判断外部渲染命令是否存在。
///
/// 参数:
/// - `name`: 命令名称
///
/// 返回:
/// - 命令是否可执行
pub(super) fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// 确认渲染输出文件已经生成。
///
/// 参数:
/// - `path`: 输出文件路径
///
/// 返回:
/// - 文件存在时成功，否则返回错误
pub(super) fn ensure_file_exists(path: &Path) -> Result<()> {
    if path.is_file() {
        Ok(())
    } else {
        bail!("renderer did not create {}", path.display())
    }
}

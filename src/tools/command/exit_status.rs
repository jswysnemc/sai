use anyhow::Result;
use std::path::{Path, PathBuf};

/// 由 stdout 日志路径推导退出码文件路径。
///
/// 参数:
/// - `stdout_log`: 标准输出日志路径
///
/// 返回:
/// - 同目录下的 `.exit` 文件
pub(super) fn exit_status_path(stdout_log: impl AsRef<Path>) -> PathBuf {
    let stdout_log = stdout_log.as_ref();
    let name = stdout_log
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("command");
    let stem = name.strip_suffix(".out.log").unwrap_or(name);
    stdout_log.with_file_name(format!("{stem}.exit"))
}

/// 在原命令结束后把退出码写入文件。
///
/// 参数:
/// - `command`: 用户原始命令
/// - `exit_log`: 退出码文件路径
///
/// 返回:
/// - 交给当前 Shell 执行的包装命令
pub(super) fn wrap_command_with_exit_status(command: &str, exit_log: &Path) -> String {
    let path = escape_shell_path(exit_log);
    #[cfg(windows)]
    {
        format!(
            "{command}; if ($null -ne $LASTEXITCODE) {{ $code = $LASTEXITCODE }} else {{ $code = [int](-not $?) }}; Set-Content -LiteralPath {path} -Value $code -NoNewline"
        )
    }
    #[cfg(not(windows))]
    {
        // 子 shell 承接 exit，避免用户命令里的 exit 跳过退出码写入
        format!("(\n{command}\n)\nprintf '%s\\n' \"$?\" > {path}")
    }
}

/// 读取已写入的退出码。
///
/// 参数:
/// - `exit_log`: 退出码文件路径
///
/// 返回:
/// - 解析成功时的退出码
pub(super) fn read_exit_code(exit_log: impl AsRef<Path>) -> Result<Option<i32>> {
    let exit_log = exit_log.as_ref();
    if !exit_log.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(exit_log)?;
    Ok(text.trim().parse().ok())
}

/// 删除退出码文件。
///
/// 参数:
/// - `stdout_log`: 对应的标准输出日志路径
///
/// 返回:
/// - 无
pub(super) fn remove_exit_status_file(stdout_log: impl AsRef<Path>) {
    let _ = std::fs::remove_file(exit_status_path(stdout_log));
}

/// 按当前平台转义路径，供包装命令写入。
///
/// 参数:
/// - `path`: 退出码文件
///
/// 返回:
/// - 可嵌入命令的路径文本
fn escape_shell_path(path: &Path) -> String {
    let value = path.display().to_string();
    #[cfg(windows)]
    {
        format!("'{}'", value.replace('\'', "''"))
    }
    #[cfg(not(windows))]
    {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证退出码路径与 stdout 日志成对。
    #[test]
    fn derives_exit_path_from_stdout_log() {
        let path = exit_status_path("/tmp/1787-paru.out.log");
        assert_eq!(path, PathBuf::from("/tmp/1787-paru.exit"));
    }

    /// 验证包装命令会写入退出码文件。
    #[test]
    fn wrap_includes_exit_file() {
        let wrapped = wrap_command_with_exit_status("false", Path::new("/tmp/cmd.exit"));
        assert!(wrapped.contains("false"));
        assert!(wrapped.contains("/tmp/cmd.exit"));
    }

    /// 验证能读出已写入的退出码。
    #[test]
    fn reads_written_exit_code() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("cmd.exit");
        std::fs::write(&path, "7\n").unwrap();
        assert_eq!(read_exit_code(&path).unwrap(), Some(7));
    }
}

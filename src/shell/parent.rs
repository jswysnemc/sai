use std::path::Path;

/// 探测当前 Sai 进程外层的用户 Shell。
///
/// 返回:
/// - 已识别的 `fish`、`bash`、`zsh` 或 `powershell`
pub(super) fn current_shell() -> Option<String> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let mut pid = std::process::id();
        for _ in 0..8 {
            let (parent, name) = parent_process(pid)?;
            if let Some(shell) = normalize_shell_name(&name) {
                return Some(shell);
            }
            pid = parent;
        }
        None
    }
    #[cfg(windows)]
    {
        windows_shell_from_environment()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        None
    }
}

/// 读取父进程 ID 和可执行文件名。
#[cfg(target_os = "linux")]
fn parent_process(pid: u32) -> Option<(u32, String)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    let mut fields = after_name.split_whitespace();
    let _state = fields.next()?;
    let parent = fields.next()?.parse().ok()?;
    let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    Some((parent, name.trim().to_string()))
}

/// 通过 macOS 的 `ps` 查询父进程，避免依赖 Linux `/proc`。
#[cfg(target_os = "macos")]
fn parent_process(pid: u32) -> Option<(u32, String)> {
    let output = std::process::Command::new("ps")
        .args(["-o", "ppid=", "-o", "comm=", "-p"])
        .arg(pid.to_string())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let parent = fields.next()?.parse().ok()?;
    let name = fields.next()?.to_string();
    Some((parent, name))
}

/// 归一化 Shell 可执行文件名。
fn normalize_shell_name(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    let name = Path::new(&normalized)
        .file_name()?
        .to_string_lossy()
        .trim_end_matches(".exe")
        .to_ascii_lowercase();
    match name.as_str() {
        "fish" | "bash" | "zsh" => Some(name),
        "pwsh" | "powershell" => Some("powershell".to_string()),
        _ => None,
    }
}

/// 从 Windows 进程环境推断当前交互 Shell。
#[cfg(windows)]
fn windows_shell_from_environment() -> Option<String> {
    if std::env::var_os("PSModulePath").is_some() {
        return Some("powershell".to_string());
    }
    std::env::var_os("SHELL").and_then(|value| normalize_shell_name(&value.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::normalize_shell_name;

    #[test]
    fn normalizes_shell_paths_for_each_supported_shell() {
        assert_eq!(normalize_shell_name("/bin/zsh"), Some("zsh".to_string()));
        assert_eq!(
            normalize_shell_name("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
            Some("powershell".to_string())
        );
        assert_eq!(normalize_shell_name("cmd.exe"), None);
    }
}

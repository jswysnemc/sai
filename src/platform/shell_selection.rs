use std::ffi::{OsStr, OsString};

/// 根据 Windows 环境和候选程序选择交互式 Shell。
///
/// 参数:
/// - `configured`: 用户配置的网页终端 Shell
/// - `shell`: `SHELL` 环境变量
/// - `pwsh_available`: PATH 中是否存在 PowerShell 7
/// - `powershell_available`: PATH 中是否存在 Windows PowerShell
/// - `comspec`: `COMSPEC` 环境变量
///
/// 返回:
/// - 交互式 Shell 程序
pub(super) fn select_windows_interactive_shell(
    configured: Option<&OsStr>,
    shell: Option<&OsStr>,
    pwsh_available: bool,
    powershell_available: bool,
    comspec: Option<&OsStr>,
) -> OsString {
    // 【Windows终端】【选择Shell】1. 优先使用用户明确配置的 Shell
    if let Some(configured) = configured.filter(|value| !value.is_empty()) {
        return configured.to_owned();
    }
    // 【Windows终端】【选择Shell】2. 默认优先选择可用的 PowerShell
    if pwsh_available {
        return OsString::from("pwsh.exe");
    }
    if powershell_available {
        return OsString::from("powershell.exe");
    }
    // 【Windows终端】【选择Shell】3. PowerShell 不可用时优先使用 Windows 系统命令解释器
    if let Some(comspec) = comspec.filter(|value| !value.is_empty()) {
        return comspec.to_owned();
    }
    // 【Windows终端】【选择Shell】4. 最后才采用可能来自 MSYS 或 WSL 的 SHELL
    shell
        .filter(|value| !value.is_empty())
        .map(OsStr::to_owned)
        .unwrap_or_else(|| OsString::from("cmd.exe"))
}

/// 返回 Windows 交互式 Shell 的启动参数。
///
/// 参数:
/// - `program`: 已选中的 Shell 程序
///
/// 返回:
/// - PowerShell 隐藏启动横幅，其他 Shell 不附加参数
pub(super) fn windows_interactive_shell_args(program: &OsStr) -> Vec<OsString> {
    let name = std::path::Path::new(program)
        .file_name()
        .unwrap_or(program)
        .to_string_lossy()
        .to_ascii_lowercase();
    if matches!(
        name.as_str(),
        "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe"
    ) {
        vec![OsString::from("-NoLogo")]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_terminal_prefers_powershell_over_posix_shell_environment() {
        let selected = select_windows_interactive_shell(
            None,
            Some(OsStr::new("/usr/bin/bash")),
            true,
            true,
            Some(OsStr::new("cmd.exe")),
        );

        assert_eq!(selected, OsString::from("pwsh.exe"));
    }

    #[test]
    fn windows_terminal_falls_back_to_windows_powershell() {
        let selected = select_windows_interactive_shell(None, None, false, true, None);

        assert_eq!(selected, OsString::from("powershell.exe"));
    }

    #[test]
    fn windows_terminal_starts_powershell_interactively_without_logo() {
        let args = windows_interactive_shell_args(OsStr::new("powershell.exe"));

        assert_eq!(args, vec![OsString::from("-NoLogo")]);
    }

    #[test]
    fn windows_terminal_prefers_comspec_over_posix_shell() {
        let selected = select_windows_interactive_shell(
            None,
            Some(OsStr::new("/usr/bin/bash")),
            false,
            false,
            Some(OsStr::new("C:\\Windows\\System32\\cmd.exe")),
        );

        assert_eq!(selected, OsString::from("C:\\Windows\\System32\\cmd.exe"));
    }

    /// 验证 Windows 网页终端优先使用用户配置的 Shell。
    #[test]
    fn windows_terminal_prefers_configured_shell() {
        let selected = select_windows_interactive_shell(
            Some(OsStr::new("C:\\Program Files\\PowerShell\\7\\pwsh.exe")),
            None,
            true,
            true,
            Some(OsStr::new("cmd.exe")),
        );

        assert_eq!(
            selected,
            OsString::from("C:\\Program Files\\PowerShell\\7\\pwsh.exe")
        );
    }
}

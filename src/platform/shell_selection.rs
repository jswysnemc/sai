use std::ffi::{OsStr, OsString};

/// Shell 的参数风格。
///
/// 按风格而不是按程序名分派：同一种风格可能对应多个程序名
/// （pwsh 与 powershell、sh 与 bash），判定与用法分开才不会两处各写一套。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShellFlavor {
    /// 传统 Windows 命令解释器
    Cmd,
    /// Windows PowerShell 或 PowerShell 7
    PowerShell,
    /// POSIX 兼容 Shell
    Posix,
    /// 程序名无法对应到已知风格
    Unknown,
}

impl ShellFlavor {
    /// 判断执行脚本时是否套用 PowerShell 语法。
    ///
    /// 未识别的程序也归入此类：Windows 上默认选出来的 Shell 就是 PowerShell，
    /// 而误按 cmd 语法拼接会让稍复杂的脚本静默地以错误方式执行。此处必须下注，
    /// 与交互式启动不同。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否套用 PowerShell 语法
    pub(crate) fn uses_powershell_syntax(self) -> bool {
        matches!(self, Self::PowerShell | Self::Unknown)
    }
}

/// 按程序路径判断 Shell 风格。
///
/// 认不出来就如实返回 `Unknown`，不替调用方猜：执行脚本时必须选一种语法，
/// 交互式启动时却不必附加任何参数，两者对未知程序的正确处理并不相同。
///
/// 参数:
/// - `program`: Shell 程序路径或名称
///
/// 返回:
/// - Shell 风格
pub(crate) fn shell_flavor(program: &OsStr) -> ShellFlavor {
    match program_name(program).as_str() {
        "cmd" | "cmd.exe" => ShellFlavor::Cmd,
        "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe" => ShellFlavor::PowerShell,
        "sh" | "sh.exe" | "bash" | "bash.exe" | "zsh" | "zsh.exe" | "fish" | "fish.exe" => {
            ShellFlavor::Posix
        }
        _ => ShellFlavor::Unknown,
    }
}

/// 按风格生成执行脚本文本的参数。
///
/// 参数:
/// - `flavor`: Shell 风格
/// - `script`: 脚本文本
///
/// 返回:
/// - 参数列表，末项为脚本本身
pub(crate) fn script_args(flavor: ShellFlavor, script: &str) -> Vec<OsString> {
    match flavor {
        // /S 保留引号原样，否则首尾引号会被 cmd 吃掉
        ShellFlavor::Cmd => vec![
            OsString::from("/S"),
            OsString::from("/C"),
            OsString::from(script),
        ],
        ShellFlavor::Posix => vec![OsString::from("-lc"), OsString::from(script)],
        // 关掉横幅、配置文件与交互提示：钩子与工具调用都在非交互场景下运行，
        // 加载用户 profile 既慢又可能因 profile 报错让脚本整体失败。
        // Unknown 一并落这里，理由见 uses_powershell_syntax
        ShellFlavor::PowerShell | ShellFlavor::Unknown => vec![
            OsString::from("-NoLogo"),
            OsString::from("-NoProfile"),
            OsString::from("-NonInteractive"),
            OsString::from("-Command"),
            OsString::from(script),
        ],
    }
}

/// 返回程序文件名的小写形式。
///
/// 两种分隔符都显式处理，不借用 `Path::file_name`：后者的语义随目标平台
/// 变化，同一个 `C:\...\cmd.exe` 在 Linux 上会被当成单段文件名。这既让
/// 逻辑能在非 Windows 上被测到，也覆盖了 Windows 上 SHELL 可能是
/// `/usr/bin/bash` 这种 MSYS 风格路径的情况。
///
/// 参数:
/// - `program`: 程序路径或名称
///
/// 返回:
/// - 小写文件名
fn program_name(program: &OsStr) -> String {
    program
        .to_string_lossy()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

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
/// 只有确认是 PowerShell 才附加参数。交互式启动不需要猜：不带参数对任何 Shell
/// 都能起来，而把 -NoLogo 传给一个不认识它的程序会让终端直接起不来。用户配置
/// 的 nu、elvish 等 Shell 正是走到这里，拿启动失败换一行横幅并不划算。
///
/// 参数:
/// - `program`: 已选中的 Shell 程序
///
/// 返回:
/// - PowerShell 隐藏启动横幅，其他 Shell 不附加参数
pub(super) fn windows_interactive_shell_args(program: &OsStr) -> Vec<OsString> {
    if shell_flavor(program) == ShellFlavor::PowerShell {
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

    /// 验证两种 PowerShell 与带路径的形式都判成同一风格。
    #[test]
    fn every_powershell_spelling_maps_to_one_flavor() {
        for program in [
            "pwsh",
            "pwsh.exe",
            "powershell.exe",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
        ] {
            assert_eq!(shell_flavor(OsStr::new(program)), ShellFlavor::PowerShell);
        }
    }

    /// 验证 cmd 与 POSIX Shell 各归其类。
    #[test]
    fn cmd_and_posix_shells_are_classified_separately() {
        assert_eq!(shell_flavor(OsStr::new("cmd.exe")), ShellFlavor::Cmd);
        assert_eq!(
            shell_flavor(OsStr::new(r"C:\Windows\System32\cmd.exe")),
            ShellFlavor::Cmd
        );
        assert_eq!(shell_flavor(OsStr::new("/bin/bash")), ShellFlavor::Posix);
        assert_eq!(shell_flavor(OsStr::new("zsh.exe")), ShellFlavor::Posix);
    }

    /// 验证无法识别的程序如实归为未知。
    #[test]
    fn an_unknown_program_is_reported_as_unknown() {
        assert_eq!(
            shell_flavor(OsStr::new("some-custom-shell.exe")),
            ShellFlavor::Unknown
        );
    }

    /// 验证未知程序执行脚本时仍按 PowerShell 语法。
    ///
    /// 误判成 cmd 会让脚本以错误语法执行，而且不报错。
    #[test]
    fn an_unknown_program_still_runs_scripts_as_powershell() {
        assert!(ShellFlavor::Unknown.uses_powershell_syntax());
        assert_eq!(
            script_args(ShellFlavor::Unknown, "echo sai"),
            script_args(ShellFlavor::PowerShell, "echo sai")
        );
    }

    /// 验证交互式启动未知 Shell 时不附加任何参数。
    ///
    /// 用户配置的 nu、elvish 等不认识 -NoLogo，传过去终端根本起不来。
    #[test]
    fn an_unknown_interactive_shell_gets_no_arguments() {
        assert!(windows_interactive_shell_args(OsStr::new("nu.exe")).is_empty());
        assert!(windows_interactive_shell_args(OsStr::new("custom-shell")).is_empty());
    }

    /// 验证每种风格的参数末项都是脚本本身。
    #[test]
    fn the_script_is_always_the_final_argument() {
        for flavor in [ShellFlavor::Cmd, ShellFlavor::PowerShell, ShellFlavor::Posix] {
            let args = script_args(flavor, "echo sai");

            assert_eq!(args.last().unwrap(), OsStr::new("echo sai"));
        }
    }

    /// 验证 PowerShell 以非交互方式执行且不加载用户配置。
    ///
    /// 少了 -NoProfile，用户 profile 里的任何报错都会让钩子整体失败。
    #[test]
    fn powershell_runs_non_interactively_without_a_profile() {
        let args = script_args(ShellFlavor::PowerShell, "echo sai");

        assert!(args.contains(&OsString::from("-NoProfile")));
        assert!(args.contains(&OsString::from("-NonInteractive")));
        assert!(args.contains(&OsString::from("-Command")));
    }

    /// 验证 cmd 保留脚本中的引号。
    ///
    /// 缺 /S 时 cmd 会剥掉首尾引号，带引号路径的命令就此断成两段。
    #[test]
    fn cmd_preserves_quotes_in_the_script() {
        let args = script_args(ShellFlavor::Cmd, r#""C:\Program Files\app.exe" --flag"#);

        assert_eq!(args[0], OsString::from("/S"));
        assert_eq!(args[1], OsString::from("/C"));
    }

    /// 验证 POSIX Shell 走登录模式的单条命令。
    #[test]
    fn posix_shells_use_a_login_command() {
        assert_eq!(
            script_args(ShellFlavor::Posix, "echo sai")[0],
            OsString::from("-lc")
        );
    }
}

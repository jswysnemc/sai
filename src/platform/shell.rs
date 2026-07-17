use std::ffi::OsString;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::process::Command;

/// Shell 命令调用参数。
#[derive(Debug, Clone)]
pub(crate) struct ShellInvocation {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
}

/// 构造当前平台执行脚本文本的 Shell 调用。
///
/// 参数:
/// - `script`: 需要执行的 Shell 脚本文本
///
/// 返回:
/// - Shell 程序及参数
pub(crate) fn command_invocation(script: &str) -> ShellInvocation {
    let program = preferred_shell_program();
    let args = command_args(&program, script);
    ShellInvocation { program, args }
}

/// 返回网页终端应使用的 Shell 调用。
///
/// 参数:
/// - `configured_shell`: 用户配置的 Shell 可执行文件路径或名称
///
/// 返回:
/// - Shell 程序和交互启动参数；`None` 表示使用用户登录 Shell
pub(crate) fn terminal_shell_invocation(configured_shell: &str) -> Option<ShellInvocation> {
    let configured_shell = configured_shell.trim();
    #[cfg(windows)]
    {
        let configured = (!configured_shell.is_empty()).then(|| OsString::from(configured_shell));
        let program = super::shell_selection::select_windows_interactive_shell(
            configured.as_deref(),
            std::env::var_os("SHELL").as_deref(),
            executable_in_path("pwsh.exe").is_some(),
            executable_in_path("powershell.exe").is_some(),
            std::env::var_os("COMSPEC").as_deref(),
        );
        let args = super::shell_selection::windows_interactive_shell_args(&program);
        return Some(ShellInvocation { program, args });
    }
    #[cfg(not(windows))]
    {
        if configured_shell.is_empty() {
            return None;
        }
        Some(ShellInvocation {
            program: OsString::from(configured_shell),
            args: Vec::new(),
        })
    }
}

/// 构造外部编辑器启动命令。
///
/// 参数:
/// - `editor`: 用户配置的编辑器命令
/// - `path`: 待编辑文件路径
///
/// 返回:
/// - 已配置的进程命令
pub(crate) fn editor_command(editor: &str, path: &Path) -> Command {
    #[cfg(windows)]
    {
        let program = preferred_shell_program();
        let script = if is_powershell(&program) {
            format!("& {editor} {}", powershell_quote(path))
        } else {
            format!("{editor} {}", cmd_quote(path))
        };
        let mut command = Command::new(&program);
        command.args(command_args(&program, &script));
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!("{} \"$1\"", editor))
            .arg("sai-editor")
            .arg(path);
        command
    }
}

/// 返回当前平台默认编辑器命令。
///
/// 返回:
/// - 默认编辑器名称
pub(crate) fn default_editor() -> &'static str {
    #[cfg(windows)]
    {
        "notepad.exe"
    }
    #[cfg(not(windows))]
    {
        "vi"
    }
}

/// 选择当前环境可用的 Shell 程序。
///
/// 返回:
/// - Shell 程序路径或名称
fn preferred_shell_program() -> OsString {
    #[cfg(windows)]
    {
        // 1. 优先尊重显式 SHELL 配置
        if let Some(shell) = non_empty_env("SHELL") {
            return shell;
        }
        // 2. 优先选择现代 PowerShell，其次选择系统 PowerShell
        for candidate in ["pwsh.exe", "powershell.exe"] {
            if executable_in_path(candidate).is_some() {
                return OsString::from(candidate);
            }
        }
        // 3. 回退到 Windows 保证存在的命令解释器
        non_empty_env("COMSPEC").unwrap_or_else(|| OsString::from("cmd.exe"))
    }
    #[cfg(not(windows))]
    {
        non_empty_env("SHELL").unwrap_or_else(|| {
            if Path::new("/bin/zsh").exists() {
                OsString::from("/bin/zsh")
            } else {
                OsString::from("/bin/sh")
            }
        })
    }
}

/// 根据 Shell 类型生成脚本执行参数。
///
/// 参数:
/// - `program`: Shell 程序
/// - `script`: 脚本文本
///
/// 返回:
/// - Shell 参数列表
fn command_args(program: &OsString, script: &str) -> Vec<OsString> {
    #[cfg(windows)]
    {
        if is_cmd(program) {
            vec![OsString::from("/S"), OsString::from("/C"), script.into()]
        } else if is_posix_shell(program) {
            vec![OsString::from("-lc"), script.into()]
        } else {
            vec![
                OsString::from("-NoLogo"),
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                script.into(),
            ]
        }
    }
    #[cfg(not(windows))]
    {
        let _ = program;
        vec![OsString::from("-lc"), script.into()]
    }
}

/// 读取非空环境变量。
///
/// 参数:
/// - `name`: 环境变量名称
///
/// 返回:
/// - 非空环境变量值
fn non_empty_env(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

/// 在 PATH 中查找可执行文件。
///
/// 参数:
/// - `name`: 可执行文件名
///
/// 返回:
/// - 已找到的完整路径
#[cfg(windows)]
fn executable_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// 判断程序是否为传统 Windows 命令解释器。
///
/// 参数:
/// - `program`: Shell 程序
///
/// 返回:
/// - 是否为 cmd
#[cfg(windows)]
fn is_cmd(program: &OsString) -> bool {
    matches!(program_name(program).as_str(), "cmd" | "cmd.exe")
}

/// 判断程序是否为 PowerShell。
///
/// 参数:
/// - `program`: Shell 程序
///
/// 返回:
/// - 是否为 PowerShell
#[cfg(windows)]
fn is_powershell(program: &OsString) -> bool {
    matches!(
        program_name(program).as_str(),
        "pwsh" | "pwsh.exe" | "powershell" | "powershell.exe"
    )
}

/// 判断程序是否使用 POSIX Shell 参数。
///
/// 参数:
/// - `program`: Shell 程序
///
/// 返回:
/// - 是否为 POSIX Shell
#[cfg(windows)]
fn is_posix_shell(program: &OsString) -> bool {
    matches!(
        program_name(program).as_str(),
        "sh" | "sh.exe" | "bash" | "bash.exe" | "zsh" | "zsh.exe" | "fish" | "fish.exe"
    )
}

/// 返回程序文件名的小写形式。
///
/// 参数:
/// - `program`: 程序路径或名称
///
/// 返回:
/// - 小写文件名
#[cfg(windows)]
fn program_name(program: &OsString) -> String {
    Path::new(program)
        .file_name()
        .unwrap_or(program.as_os_str())
        .to_string_lossy()
        .to_ascii_lowercase()
}

/// 将路径转换为 PowerShell 单引号字符串。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - PowerShell 字符串字面量
#[cfg(windows)]
fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

/// 将路径转换为 cmd 双引号字符串。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - cmd 字符串字面量
#[cfg(windows)]
fn cmd_quote(path: &Path) -> String {
    format!("\"{}\"", path.display().to_string().replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_invocation_contains_script() {
        let invocation = command_invocation("echo sai");

        assert!(invocation.args.iter().any(|arg| arg == "echo sai"));
    }

    #[test]
    fn default_editor_is_not_empty() {
        assert!(!default_editor().is_empty());
    }

    /// 验证 Unix 空配置会交由 PTY 解析用户登录 Shell。
    #[cfg(not(windows))]
    #[test]
    fn empty_terminal_shell_uses_user_login_shell() {
        assert!(terminal_shell_invocation("").is_none());
    }

    /// 验证 Unix 网页终端使用用户配置的 Shell。
    #[cfg(not(windows))]
    #[test]
    fn configured_terminal_shell_is_used() {
        let invocation = terminal_shell_invocation("/bin/bash").expect("应返回配置 Shell");

        assert_eq!(invocation.program, OsString::from("/bin/bash"));
        assert!(invocation.args.is_empty());
    }
}

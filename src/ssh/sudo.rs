//! 识别需要用户输入的 sudo，并把命令改成从 stdin 读密码。
//!
//! 模型只能提交「要跑哪条带 sudo 的命令」，不能提交密码。密码由前端安全输入
//! 后经秘密通道到达后端，再以 `sudo -S` 写入远端 stdin；回显一律走脱敏。

use fancy_regex::Regex;
use std::sync::OnceLock;

/// 判断命令是否会向用户要 sudo 密码。
///
/// `sudo -n` 明确不要交互，不弹窗。其余带 `sudo` 的命令都视为需要密码。
///
/// 参数:
/// - `command`: 远端命令原文
///
/// 返回:
/// - 需要向用户征询 sudo 密码时为 `true`
pub(crate) fn needs_sudo_password(command: &str) -> bool {
    has_sudo(command) && !has_sudo_noninteractive(command)
}

/// 确保 sudo 从 stdin 读密码（插入 `-S`，已有则不动）。
///
/// 参数:
/// - `command`: 原始命令
///
/// 返回:
/// - 可把密码写到 stdin 的命令
pub(crate) fn with_sudo_stdin(command: &str) -> String {
    let Some(regex) = sudo_without_stdin_regex() else {
        return command.to_string();
    };
    regex.replace(command, "sudo -S").into_owned()
}

fn has_sudo(command: &str) -> bool {
    sudo_word_regex().is_some_and(|regex| regex.is_match(command).unwrap_or(false))
}

fn has_sudo_noninteractive(command: &str) -> bool {
    sudo_noninteractive_regex().is_some_and(|regex| regex.is_match(command).unwrap_or(false))
}

fn sudo_word_regex() -> Option<&'static Regex> {
    static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"(?i)\bsudo\b").ok())
        .as_ref()
}

fn sudo_noninteractive_regex() -> Option<&'static Regex> {
    static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"(?i)\bsudo\b(?:\s+-[A-Zab-mo-z]*)*\s+-n\b").ok())
        .as_ref()
}

fn sudo_without_stdin_regex() -> Option<&'static Regex> {
    static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
    REGEX
        .get_or_init(|| Regex::new(r"(?i)\bsudo\b(?!\s+-S\b)").ok())
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_sudo() {
        assert!(needs_sudo_password("sudo apt update"));
        assert!(needs_sudo_password("sudo -u root systemctl restart nginx"));
        assert!(needs_sudo_password("cd /opt && sudo make install"));
    }

    #[test]
    fn skips_noninteractive_sudo() {
        assert!(!needs_sudo_password("sudo -n true"));
        assert!(!needs_sudo_password("sudo -n apt-get update"));
        assert!(!needs_sudo_password("ls /var/log"));
    }

    #[test]
    fn inserts_dash_s_once() {
        assert_eq!(with_sudo_stdin("sudo apt update"), "sudo -S apt update");
        assert_eq!(with_sudo_stdin("sudo -S apt update"), "sudo -S apt update");
        assert_eq!(with_sudo_stdin("sudo -u root id"), "sudo -S -u root id");
    }
}

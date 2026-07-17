pub mod bash;
pub mod fish;
pub mod powershell;
pub mod zsh;

use crate::i18n::text as t;
use std::path::Path;

pub fn print_reload_hint(shell: &str, hook_file: &Path) {
    let source = match shell {
        "fish" => format!("source {}", fish_quote(hook_file)),
        "bash" | "zsh" => format!("source {}", shell_quote(hook_file)),
        "powershell" => format!(". {}", powershell_quote(hook_file)),
        _ => return,
    };
    if current_parent_shell().as_deref() == Some(shell) {
        println!(
            "{}: {}",
            t(
                "run this in the current terminal to load it now",
                "在当前终端运行此命令可立即加载"
            ),
            source
        );
    } else {
        println!(
            "{}",
            t(
                "open a new matching shell session for the hook to take effect",
                "新开对应 shell 会话后 hook 将生效"
            )
        );
    }
}

pub fn current_parent_shell() -> Option<String> {
    let mut pid = std::process::id();
    for _ in 0..8 {
        let parent = parent_pid(pid)?;
        let name = process_name(parent)?;
        if matches!(
            name.as_str(),
            "fish" | "bash" | "zsh" | "pwsh" | "powershell"
        ) {
            if matches!(name.as_str(), "pwsh" | "powershell") {
                return Some("powershell".to_string());
            }
            return Some(name);
        }
        pid = parent;
    }
    None
}

fn parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after_name = stat.rsplit_once(") ")?.1;
    after_name.split_whitespace().nth(1)?.parse().ok()
}

fn process_name(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

fn fish_quote(path: &Path) -> String {
    format!(
        "'{}'",
        path.display()
            .to_string()
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
    )
}

fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "''"))
}

pub fn looks_like_natural_language(input: &str) -> bool {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return false;
    }
    !trimmed.contains('\n') && !trimmed.contains('\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_safe_natural_language() {
        assert!(looks_like_natural_language("帮我查一下 niri 输入法"));
        assert!(looks_like_natural_language(
            "why is fcitx candidate window small"
        ));
    }

    #[test]
    fn accepts_command_not_found_text_without_syntax_filtering() {
        assert!(looks_like_natural_language(
            "这样写可以吗？假设我们输入一个字母`x`"
        ));
        assert!(looks_like_natural_language(
            "我好像在输入里加一个左斜杠就会导致输入不被传给sai/对吗？"
        ));
        assert!(looks_like_natural_language(
            "软件需要适配 Wayland 的 `text-input` 协议，输入法要支持 $GTK_IM_MODULE 吗？"
        ));
        assert!(looks_like_natural_language(
            "GTK_IM_MODULE=fcitx 是什么意思？"
        ));
        assert!(looks_like_natural_language(
            "./target/release/sai 查询为什么失败？"
        ));
    }

    #[test]
    fn rejects_empty_or_multiline_text() {
        assert!(!looks_like_natural_language(""));
        assert!(!looks_like_natural_language("   "));
        assert!(!looks_like_natural_language("第一行\n第二行"));
    }
}

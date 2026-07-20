pub mod bash;
pub mod fish;
pub(crate) mod intercept_store;
mod parent;
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
    parent::current_shell()
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

#[cfg_attr(not(test), allow(dead_code))]
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

use crate::render::style::TOOL_BULLET;
use crate::render::terminal_text as t;

/// REPL 本地 Shell 命令单元。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShellCell {
    pub(crate) command: String,
    pub(crate) output: String,
    pub(crate) exit_code: Option<i32>,
}

/// 渲染本地 Shell 命令和输出。
///
/// 参数:
/// - `cell`: Shell 命令快照
///
/// 返回:
/// - 适合 transcript 的 ANSI 文本
pub(super) fn render(cell: &ShellCell) -> String {
    // Codex 风格：• You ran $ command
    let mut rendered = format!(
        "\x1b[1m\x1b[32m{TOOL_BULLET}\x1b[0m \x1b[1m{}\x1b[0m \x1b[35m$\x1b[0m {}",
        t("You ran", "已执行"),
        cell.command
    );
    if cell.output.is_empty() {
        rendered.push_str("\n\x1b[2m  └ (no output)\x1b[0m");
    } else {
        for (index, line) in cell.output.lines().enumerate() {
            let prefix = if index == 0 { "  └ " } else { "    " };
            rendered.push_str(&format!("\n\x1b[2m{prefix}{line}\x1b[0m"));
        }
    }
    if cell.exit_code.is_some_and(|code| code != 0) {
        rendered.push_str(&format!(
            "\n\x1b[31m✗ ({})\x1b[0m",
            cell.exit_code.unwrap_or_default()
        ));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_command_output_without_tree_prefixes() {
        let rendered = render(&ShellCell {
            command: "ls".to_string(),
            output: "one\ntwo\n".to_string(),
            exit_code: Some(0),
        });

        assert!(rendered.contains("ls"));
        assert!(rendered.contains("one"));
        assert!(rendered.contains("two"));
        assert!(!rendered.contains("├─"));
        assert!(!rendered.contains("└─"));
        assert!(rendered.contains("└"));
    }
}

use crate::render::terminal_text as t;
use crate::render::style::TOOL_BULLET;

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
    let mut rendered = format!("{TOOL_BULLET} {} {}", t("You ran", "已执行"), cell.command);
    if cell.output.is_empty() {
        rendered.push_str(&format!(
            "\n\x1b[2m{}\x1b[0m",
            t("no output", "无输出")
        ));
    } else {
        rendered.push('\n');
        rendered.push_str(&cell.output);
    }
    if cell.exit_code.is_some_and(|code| code != 0) {
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        rendered.push_str(&format!(
            "\x1b[31m  {} {}\x1b[0m",
            t("exit code", "退出码"),
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
        assert!(rendered.contains("\none\ntwo"));
        assert!(!rendered.contains("├─"));
        assert!(!rendered.contains("└─"));
    }
}

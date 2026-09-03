use crate::render::code_block::{highlight_code_line_continued, CodeLineHighlightState};
use crate::render::fold_text::{
    command_body_column, command_wrap_width_for_title, fold_display_lines,
    fold_display_lines_tracked, wrap_display_lines, FoldedDisplayLine, FOLD_HEAD_LINES,
    FOLD_TAIL_LINES,
};
use crate::render::status_style::{tool_bullet, ToolHealth};
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
    // 1. 标题 + 折叠后的命令（过长收缩，保留 shell 着色）
    //    中文标题占四列而英文占七列，折行宽度与续行缩进都必须按实际标题算，
    //    否则首行会超出终端宽度、被硬换行到视觉引导线所在的第 0 列
    let title = t("You ran", "已执行");
    // 圆点颜色与工具卡同语义：零退出码绿、非零红
    let health = match cell.exit_code {
        Some(code) if code != 0 => ToolHealth::Err,
        _ => ToolHealth::Ok,
    };
    let command_lines = fold_command_entries(cell.command.trim(), false, title);
    let mut rendered = format!("{} \x1b[1m{title}\x1b[0m ", tool_bullet(health));
    if command_lines.is_empty() {
        rendered.push_str("\x1b[35m$\x1b[0m ");
    } else {
        // 折行续行共享高亮状态，跨行字符串不会被重新 token 化
        let mut highlight = CodeLineHighlightState::default();
        let continuation = " ".repeat(command_body_column(title));
        let mut first = true;
        for entry in &command_lines {
            if first {
                rendered.push_str("\x1b[35m$\x1b[0m ");
            } else {
                rendered.push_str(&continuation);
            }
            push_shell_line(&mut rendered, entry, &mut highlight);
            first = false;
        }
    }
    // 2. 输出体：过长结果同样首尾折叠
    if cell.output.is_empty() {
        rendered.push_str("\n\x1b[2m  └ (no output)\x1b[0m");
    } else {
        let out_lines = fold_display_text(cell.output.trim_end(), false, title);
        for (index, line) in out_lines.iter().enumerate() {
            let prefix = if index == 0 { "  └ " } else { "    " };
            if line.starts_with('…') {
                rendered.push_str(&format!("\n\x1b[2m{prefix}{line}\x1b[0m"));
            } else {
                rendered.push_str(&format!("\n\x1b[2m{prefix}{line}\x1b[0m"));
            }
        }
    }
    // 3. 非零退出码标记
    if cell.exit_code.is_some_and(|code| code != 0) {
        rendered.push_str(&format!(
            "\n\x1b[31m✗ ({})\x1b[0m",
            cell.exit_code.unwrap_or_default()
        ));
    }
    rendered
}

/// 折行并按预览行数折叠纯文本。
///
/// 参数:
/// - `text`: 原文
/// - `expanded`: 是否展开
/// - `title`: 命令块标题，决定行首占用的列数
///
/// 返回:
/// - 可见行（省略标记已本地化文案）
fn fold_display_text(text: &str, expanded: bool, title: &str) -> Vec<String> {
    // 命令与输出共用同一套折行宽度：前 2 后 4 行做预览折叠
    let wrap = command_wrap_width_for_title(title);
    let wrapped = wrap_display_lines(text, wrap);
    let (visible, omitted) =
        fold_display_lines(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded);
    visible
        .into_iter()
        .map(|line| {
            if line == "__OMITTED__" {
                crate::render::omitted_line::render_omitted_line_plain(omitted, true)
            } else {
                line
            }
        })
        .collect()
}

/// 折行并折叠命令文本，保留被省略行供高亮状态推进。
///
/// 参数:
/// - `text`: 命令原文
/// - `expanded`: 是否展开
/// - `title`: 命令块标题，决定行首占用的列数
///
/// 返回:
/// - 折叠后的显示条目
fn fold_command_entries(text: &str, expanded: bool, title: &str) -> Vec<FoldedDisplayLine> {
    let wrap = command_wrap_width_for_title(title);
    let wrapped = wrap_display_lines(text, wrap);
    fold_display_lines_tracked(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded)
}

/// 追加命令显示行。
///
/// 参数:
/// - `rendered`: 输出缓冲
/// - `entry`: 折叠后的显示条目
/// - `highlight`: 跨显示行的高亮状态
fn push_shell_line(
    rendered: &mut String,
    entry: &FoldedDisplayLine,
    highlight: &mut CodeLineHighlightState,
) {
    match entry {
        FoldedDisplayLine::Omitted { omitted, skipped } => {
            // 被省略的行仍要推进高亮状态，尾部行的引号/注释上下文才正确
            for line in skipped {
                let _ = highlight_code_line_continued("bash", line, highlight);
            }
            rendered.push_str(&crate::render::omitted_line::render_omitted_line_plain(
                *omitted, true,
            ));
            rendered.push('\n');
        }
        FoldedDisplayLine::Line(line) => {
            rendered.push_str(&highlight_code_line_continued("bash", line, highlight));
            rendered.push('\n');
        }
    }
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

    #[test]
    fn folds_long_command_in_transcript() {
        // 折行宽度跟随终端列数(宽终端下 800 字符只折出 5 行,不足
        // 头 2 + 尾 4 的折叠阈值),加长到稳定超过阈值
        let long = "echo ".to_string() + &"a".repeat(3_000);
        let rendered = render(&ShellCell {
            command: long,
            output: String::new(),
            exit_code: Some(0),
        });
        assert!(
            rendered.contains("…") || rendered.contains("lines"),
            "long command should fold: {rendered}"
        );
    }

    #[test]
    fn folds_long_output_body() {
        let output = (0..40)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render(&ShellCell {
            command: "seq".to_string(),
            output,
            exit_code: Some(0),
        });
        assert!(rendered.contains("line-0"));
        assert!(rendered.contains("…") || rendered.contains("lines"));
    }
}

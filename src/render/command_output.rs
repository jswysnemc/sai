use crate::render::code_block::{highlight_code_line_continued, CodeLineHighlightState};
use crate::render::command_result_block::truncate_chars;
pub(crate) use crate::render::command_result_block::{
    render_command_error_view_for_cli, render_command_result_view_for_cli,
};
use crate::render::status_style::{tool_bullet, ToolHealth};
use anyhow::Result;
use serde_json::Value;
use std::io::{self, Write};

/// 写入普通工具参数或输出块（统一 gutter：`  └ ` 首行 + 四空格续行）。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `label`: 输出块标签
/// - `payload`: 原始工具载荷
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_tool_payload(
    stdout: &mut io::Stdout,
    label: &str,
    payload: &str,
) -> Result<()> {
    let formatted = format_tool_payload(payload);
    let mut lines = formatted.lines();
    match lines.next() {
        Some(first) => writeln!(stdout, "\x1b[2m  └ {label}: {first}\x1b[0m")?,
        None => writeln!(stdout, "\x1b[2m  └ {label}:\x1b[0m")?,
    }
    for line in lines {
        writeln!(stdout, "\x1b[2m    {line}\x1b[0m")?;
    }
    Ok(())
}

/// 写入带动作标题的命令调用块。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `arguments`: 工具调用参数
/// - `action`: 命令动作展示名
/// - `health`: 状态语义，决定行首圆点颜色
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_command_block_with_action(
    stdout: &mut io::Stdout,
    arguments: &str,
    action: &str,
    health: ToolHealth,
) -> Result<()> {
    write!(
        stdout,
        "{}",
        render_command_block_with_action(arguments, action, health)
    )?;
    Ok(())
}

/// 写入普通 CLI 使用的五行命令结果摘要。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `output`: 命令工具返回的 JSON
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_command_result_preview(stdout: &mut io::Stdout, output: &str) -> Result<()> {
    write!(stdout, "{}", render_command_result_view_for_cli(output))?;
    Ok(())
}

/// 写入普通 CLI 使用的五行命令错误摘要。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `output`: 命令工具返回的 JSON 或错误文本
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_command_error_preview(stdout: &mut io::Stdout, output: &str) -> Result<()> {
    write!(stdout, "{}", render_command_error_view_for_cli(output))?;
    Ok(())
}

/// 渲染命令调用块。
///
/// 参数:
/// - `arguments`: 工具调用参数
///
/// 返回:
/// - 代码块风格的命令文本
#[cfg(test)]
fn render_command_block(arguments: &str) -> String {
    render_command_block_with_action(arguments, "", ToolHealth::Ok)
}

/// 渲染带动作标题的命令调用块。
///
/// 参数:
/// - `arguments`: 工具调用参数
/// - `action`: 命令动作展示名
/// - `health`: 状态语义，决定行首圆点颜色（进行中弱化、成功绿、失败红）
///
/// 返回:
/// - 代码块风格的命令文本
pub(crate) fn render_command_block_with_action(
    arguments: &str,
    action: &str,
    health: ToolHealth,
) -> String {
    let parsed = serde_json::from_str::<Value>(arguments).ok();
    let command = parsed
        .as_ref()
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .map(str::to_string)
        // 参数流式期间 JSON 尚未闭合，宽松提取已收到的命令文本，避免展示原始 JSON
        .or_else(|| crate::render::tool_event_line::lenient_string_field(arguments, "command"))
        .unwrap_or_else(|| arguments.to_string());
    let command = command.trim();
    // Codex 风格：状态圆点 + 标题 + `$` 命令行，续行缩进
    // 调用方传入 Running/Ran；兼容旧 ""/"Run" 默认按已完成 Ran
    let title = match action.trim() {
        "" | "Run" => "Ran",
        "Background" => "Background",
        other => other,
    };
    // 1. 按终端宽度折行后首尾折叠，过长命令在主列表收缩
    let lines = fold_shell_command_lines(command, false, title);
    let mut output = format!("{} \x1b[1m{title}\x1b[0m ", tool_bullet(health));
    // 2. 折行续行共享高亮状态：跨行字符串/注释的颜色才能延续，
    //    否则 URL 中段的数字会被重新 token 化成数字色
    let mut highlight = CodeLineHighlightState::default();
    if lines.is_empty() {
        output.push_str("\x1b[35m$ \x1b[0m\n");
        return output;
    }
    let continuation = " ".repeat(crate::render::fold_text::command_body_column(title));
    let mut first = true;
    for entry in &lines {
        if first {
            output.push_str("\x1b[35m$ \x1b[0m");
        } else {
            output.push_str(&continuation);
        }
        append_command_display_line(&mut output, entry, &mut highlight);
        first = false;
    }
    output
}

/// 将命令文本折行并按预览预算折叠。
///
/// 参数:
/// - `command`: 原始命令
/// - `expanded`: 是否展开全文
/// - `title`: 命令块标题，决定正文起始列
///
/// 返回:
/// - 折叠后的显示条目（占位处保留被省略行供状态推进）
fn fold_shell_command_lines(
    command: &str,
    expanded: bool,
    title: &str,
) -> Vec<crate::render::fold_text::FoldedDisplayLine> {
    use crate::render::fold_text::{
        command_wrap_width_for_title, fold_display_lines_tracked, wrap_display_lines,
        FOLD_HEAD_LINES, FOLD_TAIL_LINES,
    };
    // 命令行预览：前 2 后 4，过长时收缩
    let wrap = command_wrap_width_for_title(title);
    let wrapped = wrap_display_lines(command, wrap);
    fold_display_lines_tracked(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded)
}

/// 追加一行命令显示（省略行 dim，普通行 shell 着色）。
///
/// 参数:
/// - `output`: 输出缓冲
/// - `entry`: 折叠后的显示条目
/// - `highlight`: 跨显示行的高亮状态
fn append_command_display_line(
    output: &mut String,
    entry: &crate::render::fold_text::FoldedDisplayLine,
    highlight: &mut CodeLineHighlightState,
) {
    use crate::render::fold_text::FoldedDisplayLine;
    match entry {
        FoldedDisplayLine::Omitted { omitted, skipped } => {
            // 被省略的行仍要推进高亮状态，尾部行的引号/注释上下文才正确
            for line in skipped {
                let _ = highlight_code_line_continued("sh", line, highlight);
            }
            output.push_str(&crate::render::omitted_line::render_omitted_line_plain(
                *omitted, true,
            ));
            output.push('\n');
        }
        FoldedDisplayLine::Line(line) => {
            output.push_str(&highlight_code_line_continued("sh", line, highlight));
            output.push('\n');
        }
    }
}

/// 生成命令代码块行。
///
/// 参数:
/// - `command`: 原始命令文本
///
/// 返回:
/// - 命令行列表

/// 格式化工具载荷并限制长度。
///
/// 参数:
/// - `payload`: 原始工具载荷
///
/// 返回:
/// - 格式化后的文本
fn format_tool_payload(payload: &str) -> String {
    let text = payload.trim();
    let formatted = serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| text.to_string());
    truncate_chars(&formatted, 2400)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_multiline_command_as_code_block() {
        let output = render_command_block(
            r#"{"command":"python3 - <<'PY'\nfrom pathlib import Path\nprint(Path('x').resolve())\nPY"}"#,
        );
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("• Ran"));
        assert!(!plain.contains("──"));
        assert!(plain.contains("python3 - <<'PY'"));
        assert!(plain.contains("$ python3") || plain.contains("python3 - <<'PY'"));
        assert!(plain.contains("from pathlib import Path"));
        assert!(plain.contains("print(Path('x').resolve())"));
        assert!(plain.contains("PY"));
        assert!(!plain.contains(",-- command"));
        assert!(!plain.contains("`--"));
    }

    #[test]
    fn renders_command_block_with_action_header() {
        let output =
            render_command_block_with_action(r#"{"command":"date"}"#, "Run", ToolHealth::Ok);
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("• Ran"));
        assert!(!plain.contains("──"));
        assert!(plain.contains("$ date") || plain.contains("date"));
        assert!(!plain.contains("Run run"));
        assert!(!plain.contains("• command\n"));
    }

    #[test]
    fn renders_background_command_block_with_distinct_header() {
        let output = render_command_block_with_action(
            r#"{"command":"sleep 1"}"#,
            "Background",
            ToolHealth::Pending,
        );
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("• Background"));
        assert!(!plain.contains("──"));
        assert!(plain.contains("sleep 1"));
        assert!(!plain.contains("Run command"));
    }

    /// 去除 ANSI 转义序列，方便断言可见文本。
    ///
    /// 参数:
    /// - `text`: 原始终端文本
    ///
    /// 返回:
    /// - 去除样式后的文本

    #[test]
    fn folds_long_ran_command_in_main_view() {
        // 折行宽度跟随终端列数(可达 196+),命令要长到足以折出超过
        // FOLD_HEAD_LINES + FOLD_TAIL_LINES 行才触发折叠
        let long = "echo ".to_string() + &"x".repeat(3_000);
        let args = format!(r#"{{"command":"{long}"}}"#);
        let output = render_command_block_with_action(&args, "Run", ToolHealth::Ok);
        let plain = strip_ansi_for_test(&output);
        assert!(
            plain.contains("…") || plain.contains("lines"),
            "expected fold: {plain}"
        );
        assert!(plain.contains("Ran") || plain.contains("$"));
    }

    /// 【终端】【命令折行】验证命令块任何一行都不超出渲染宽度。
    ///
    /// 首行前缀是 `• Ran $ ` 共八列，折行宽度却按六列扣减，
    /// 于是首行实际占到宽度加二列，被终端硬换行到第 0 列——
    /// 那正是视觉引导线所在列，续行因此出现在引导线左侧。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn command_block_lines_never_exceed_the_render_width() {
        use crate::render::render_width::with_render_width;

        let command = format!("echo {}", "x".repeat(200));
        let args = serde_json::json!({ "command": command }).to_string();
        for width in [40usize, 60, 100] {
            let rendered = with_render_width(width, || {
                render_command_block_with_action(&args, "Run", ToolHealth::Ok)
            });
            for line in rendered.lines() {
                let plain = strip_ansi_for_test(line);
                let visible: usize = plain
                    .chars()
                    .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
                    .sum();
                assert!(
                    visible <= width,
                    "width={width} 行宽 {visible} 超出渲染宽度: {plain:?}"
                );
            }
        }
    }

    /// 【终端】【命令折行】验证续行正文与首行正文落在同一列。
    ///
    /// 首行正文起于 `• Ran $ ` 之后的第八列，续行若按四列缩进就会
    /// 比首行左移，长命令的后半段看起来像脱离了命令块。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn wrapped_command_lines_align_with_the_first_body_column() {
        use crate::render::render_width::with_render_width;

        let command = format!("echo {}", "x".repeat(200));
        let args = serde_json::json!({ "command": command }).to_string();
        let rendered = with_render_width(60, || {
            render_command_block_with_action(&args, "Run", ToolHealth::Ok)
        });
        let plain_lines = rendered
            .lines()
            .map(strip_ansi_for_test)
            .collect::<Vec<_>>();
        let first_body_column: usize = plain_lines[0]
            .split("echo")
            .next()
            .expect("首行必须包含命令正文")
            .chars()
            .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
            .sum();

        assert!(plain_lines.len() > 1, "样例必须触发折行");
        for line in &plain_lines[1..] {
            let indent = line.chars().take_while(|ch| *ch == ' ').count();
            assert_eq!(
                indent, first_body_column,
                "续行缩进 {indent} 未对齐首行正文列 {first_body_column}: {line:?}"
            );
        }
    }

    fn strip_ansi_for_test(text: &str) -> String {
        let mut output = String::new();
        let mut escape = false;
        let mut csi = false;
        for ch in text.chars() {
            if ch == '\x1b' {
                escape = true;
                csi = false;
            } else if escape {
                if csi {
                    if (ch as u32) >= 0x40 && (ch as u32) <= 0x7e {
                        escape = false;
                    }
                } else if ch == '[' {
                    csi = true;
                } else if ch == '\\' || ch == 'm' {
                    escape = false;
                }
            } else {
                output.push(ch);
            }
        }
        output
    }
}

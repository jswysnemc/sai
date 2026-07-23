use crate::render::code_block::highlight_code_line;
use crate::render::command_result_block::truncate_chars;
pub(crate) use crate::render::command_result_block::{
    render_command_error_view_for_cli, render_command_result_view_for_cli,
};
use crate::render::style::TOOL_BULLET;
use anyhow::Result;
use serde_json::Value;
use std::io::{self, Write};

/// 写入普通工具参数或输出块。
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
    writeln!(stdout, "\x1b[2m{TOOL_BULLET} {label}:\x1b[0m")?;
    for line in formatted.lines() {
        writeln!(stdout, "\x1b[2m  {line}\x1b[0m")?;
    }
    Ok(())
}

/// 写入带动作标题的命令调用块。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `arguments`: 工具调用参数
/// - `action`: 命令动作展示名
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_command_block_with_action(
    stdout: &mut io::Stdout,
    arguments: &str,
    action: &str,
) -> Result<()> {
    write!(
        stdout,
        "{}",
        render_command_block_with_action(arguments, action)
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
    render_command_block_with_action(arguments, "")
}

/// 渲染带动作标题的命令调用块。
///
/// 参数:
/// - `arguments`: 工具调用参数
/// - `action`: 命令动作展示名
///
/// 返回:
/// - 代码块风格的命令文本
pub(crate) fn render_command_block_with_action(arguments: &str, action: &str) -> String {
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
    // 1. 按终端宽度折行后首尾折叠，过长命令在主列表收缩
    let lines = fold_shell_command_lines(command, false);
    // Codex 风格：状态圆点 + 标题 + `$` 命令行，续行缩进
    let title = match action.trim() {
        "" | "Run" => "Ran",
        "Background" => "Background",
        other => other,
    };
    let mut output = format!("\x1b[1m\x1b[32m{TOOL_BULLET}\x1b[0m \x1b[1m{title}\x1b[0m ");
    if let Some((first, rest)) = lines.split_first() {
        output.push_str("\x1b[35m$ \x1b[0m");
        append_command_display_line(&mut output, first, true);
        for line in rest {
            output.push_str("    ");
            append_command_display_line(&mut output, line, false);
        }
    } else {
        output.push_str("\x1b[35m$ \x1b[0m\n");
    }
    output
}

/// 将命令文本折行并按预览预算折叠。
///
/// 参数:
/// - `command`: 原始命令
/// - `expanded`: 是否展开全文
///
/// 返回:
/// - 可见显示行（省略处为 `… +N lines`）
fn fold_shell_command_lines(command: &str, expanded: bool) -> Vec<String> {
    use crate::render::fold_text::{
        fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES,
    };
    // 命令行预览：前 2 后 4，过长时收缩
    let wrap = terminal_wrap_width().saturating_sub(6).min(72).max(24);
    let wrapped = wrap_display_lines(command, wrap);
    let (visible, omitted) = fold_display_lines(&wrapped, FOLD_HEAD_LINES, FOLD_TAIL_LINES, expanded);
    visible
        .into_iter()
        .map(|line| {
            if line == "__OMITTED__" {
                format!("… +{omitted} lines (Ctrl+O to expand)")
            } else {
                line
            }
        })
        .collect()
}

/// 追加一行命令显示（省略行 dim，普通行 shell 着色）。
///
/// 参数:
/// - `output`: 输出缓冲
/// - `line`: 显示行
/// - `is_first`: 是否为 `$` 同行首行
fn append_command_display_line(output: &mut String, line: &str, is_first: bool) {
    if line.starts_with('…') {
        if !is_first {
            // 续行已有缩进
        }
        output.push_str("\x1b[2m");
        output.push_str(line);
        output.push_str("\x1b[0m\n");
    } else {
        output.push_str(&highlight_code_line("sh", line));
        output.push('\n');
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
        let output = render_command_block_with_action(r#"{"command":"date"}"#, "Run");
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("• Ran"));
        assert!(!plain.contains("──"));
        assert!(plain.contains("$ date") || plain.contains("date"));
        assert!(!plain.contains("Run run"));
        assert!(!plain.contains("• command\n"));
    }

    #[test]
    fn renders_background_command_block_with_distinct_header() {
        let output = render_command_block_with_action(r#"{"command":"sleep 1"}"#, "Background");
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
        let long = "echo ".to_string() + &"x".repeat(800);
        let args = format!(r#"{{"command":"{long}"}}"#);
        let output = render_command_block_with_action(&args, "Run");
        let plain = strip_ansi_for_test(&output);
        assert!(plain.contains("…") || plain.contains("lines"), "expected fold: {plain}");
        assert!(plain.contains("Ran") || plain.contains("$"));
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

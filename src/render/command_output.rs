use crate::render::code_block::{highlight_code_line, render_code_footer, render_code_header};
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
    let lines = shell_command_lines(command);
    let header = if action.trim().is_empty() {
        format!("{TOOL_BULLET} command")
    } else {
        format!("{TOOL_BULLET} {} command", action.trim())
    };
    let mut output = render_code_header(&header);
    for line in &lines {
        output.push_str(&highlight_code_line("sh", line));
        output.push('\n');
    }
    output.push_str(&render_code_footer(&lines));
    output
}

/// 生成命令代码块行。
///
/// 参数:
/// - `command`: 原始命令文本
///
/// 返回:
/// - 命令行列表
fn shell_command_lines(command: &str) -> Vec<String> {
    let mut lines = command.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

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

        assert!(plain.contains("── • command "));
        assert!(plain.contains("python3 - <<'PY'"));
        assert!(!plain.contains("$ python3"));
        assert!(plain.contains("from pathlib import Path"));
        assert!(plain.contains("print(Path('x').resolve())"));
        assert!(plain.contains("\nPY\n"));
        assert!(!plain.contains(",-- command"));
        assert!(!plain.contains("`--"));
    }

    #[test]
    fn renders_command_block_with_action_header() {
        let output = render_command_block_with_action(r#"{"command":"date"}"#, "Run");
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("── • Run command "));
        assert!(plain.contains("date"));
        assert!(!plain.contains("Run run"));
        assert!(!plain.contains("── • command "));
    }

    #[test]
    fn renders_background_command_block_with_distinct_header() {
        let output = render_command_block_with_action(r#"{"command":"sleep 1"}"#, "Background");
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("── • Background command "));
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

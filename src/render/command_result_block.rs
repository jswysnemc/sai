use crate::i18n::text as t;
use crate::render::code_block::{render_code_footer, render_code_header};
use crate::render::style::TOOL_BULLET;
use anyhow::Result;
use serde_json::Value;
use std::io::{self, Write};

/// 按命令执行结果写入 stdout 和 stderr 块。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `output`: 命令执行结果 JSON
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_command_result_blocks(stdout: &mut io::Stdout, output: &str) -> Result<()> {
    let Some(result) = parse_command_result(output) else {
        return super::command_output::write_tool_payload(stdout, t("output", "输出"), output);
    };
    if !result.stdout.trim().is_empty() {
        write_output_block(stdout, t("output", "输出"), &result.stdout)?;
    }
    if !result.stderr.trim().is_empty() {
        let label = result
            .exit_code
            .map(|code| format!("err exit {code}"))
            .unwrap_or_else(|| "err".to_string());
        write_output_block(stdout, &label, &result.stderr)?;
    } else if !result.success {
        let label = result
            .exit_code
            .map(|code| format!("err exit {code}"))
            .unwrap_or_else(|| "err".to_string());
        write_output_block(
            stdout,
            &label,
            t(
                "command failed without stderr",
                "命令失败，但没有 stderr 输出",
            ),
        )?;
    }
    Ok(())
}

/// 写入命令失败摘要块。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `output`: 命令执行结果 JSON
///
/// 返回:
/// - 写入是否成功
pub(crate) fn write_command_error_block(stdout: &mut io::Stdout, output: &str) -> Result<()> {
    let Some(result) = parse_command_result(output) else {
        return write_output_block(stdout, "err", output);
    };
    if result.success {
        return Ok(());
    }
    let label = result
        .exit_code
        .map(|code| format!("err exit {code}"))
        .unwrap_or_else(|| "err".to_string());
    let message = if result.stderr.trim().is_empty() {
        result.stdout.as_str()
    } else {
        result.stderr.as_str()
    };
    write_output_block(stdout, &label, message)
}

/// 按字符数量截断文本。
///
/// 参数:
/// - `text`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 截断后的文本
pub(crate) fn truncate_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let omitted = total - max_chars;
    format!(
        "{}\n... {} {omitted} {} ...",
        text.chars().take(max_chars).collect::<String>(),
        t("truncated", "已截断"),
        t("chars", "字符")
    )
}

/// 写入命令输出文本块。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `label`: 文本块标签
/// - `text`: 文本内容
///
/// 返回:
/// - 写入是否成功
fn write_output_block(stdout: &mut io::Stdout, label: &str, text: &str) -> Result<()> {
    let content = truncate_chars(text.trim(), 2400);
    let lines = output_block_lines(&content);
    write!(
        stdout,
        "{}",
        render_code_header(&format!("{TOOL_BULLET} {label}"))
    )?;
    stdout.flush()?;
    for line in &lines {
        writeln!(stdout, "{line}")?;
        stdout.flush()?;
    }
    write!(stdout, "{}", render_code_footer(&lines))?;
    stdout.flush()?;
    Ok(())
}

/// 渲染命令输出文本块。
///
/// 参数:
/// - `label`: 文本块标签
/// - `text`: 文本内容
///
/// 返回:
/// - 代码块风格的输出文本
pub(crate) fn render_output_block(label: &str, text: &str) -> String {
    let content = truncate_chars(text.trim(), 2400);
    let lines = output_block_lines(&content);
    let mut output = render_code_header(&format!("{TOOL_BULLET} {label}"));
    for line in &lines {
        output.push_str(line);
        output.push('\n');
    }
    output.push_str(&render_code_footer(&lines));
    output
}

/// 生成输出代码块行。
///
/// 参数:
/// - `content`: 已截断的输出文本
///
/// 返回:
/// - 输出行列表
fn output_block_lines(content: &str) -> Vec<String> {
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

struct CommandResult {
    success: bool,
    exit_code: Option<i64>,
    stdout: String,
    stderr: String,
}

/// 解析命令工具返回的 JSON 结果。
///
/// 参数:
/// - `output`: 原始 JSON 文本
///
/// 返回:
/// - 解析后的命令结果，解析失败时返回空
fn parse_command_result(output: &str) -> Option<CommandResult> {
    let value = serde_json::from_str::<Value>(output.trim()).ok()?;
    Some(CommandResult {
        success: value.get("success")?.as_bool()?,
        exit_code: value.get("exit_code").and_then(Value::as_i64),
        stdout: value
            .get("stdout")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        stderr: value
            .get("stderr")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

/// 将命令工具 JSON 结果渲染为 stdout/stderr 代码块（供 TUI 工具视图复用）。
///
/// 参数:
/// - `output`: 命令工具返回的 JSON
///
/// 返回:
/// - 可读的命令输出文本；解析失败时返回格式化后的原始载荷
pub(crate) fn render_command_result_view(output: &str) -> String {
    let Some(result) = parse_command_result(output) else {
        return render_output_block(t("output", "输出"), output);
    };
    let mut parts = Vec::new();
    if !result.stdout.trim().is_empty() {
        parts.push(render_output_block(t("output", "输出"), &result.stdout));
    }
    if !result.stderr.trim().is_empty() {
        let label = result
            .exit_code
            .map(|code| format!("err exit {code}"))
            .unwrap_or_else(|| "err".to_string());
        parts.push(render_output_block(&label, &result.stderr));
    } else if !result.success {
        let label = result
            .exit_code
            .map(|code| format!("err exit {code}"))
            .unwrap_or_else(|| "err".to_string());
        parts.push(render_output_block(
            &label,
            t(
                "command failed without stderr",
                "命令失败，但没有 stderr 输出",
            ),
        ));
    } else if result.stdout.trim().is_empty() {
        parts.push(render_output_block(
            t("output", "输出"),
            t("no output", "无输出"),
        ));
    }
    let mut joined = parts.join("");
    while joined.ends_with('\n') {
        joined.pop();
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_result_json() {
        let result = parse_command_result(
            r#"{"success":false,"exit_code":1,"stdout":"unused","stderr":"not found"}"#,
        )
        .unwrap();
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
        assert_eq!(result.stdout, "unused");
        assert_eq!(result.stderr, "not found");
    }

    #[test]
    fn renders_command_output_as_code_block() {
        let output = render_output_block("output", "sent to snemc@qq.com\n");
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("── • output "));
        assert!(plain.contains("sent to snemc@qq.com"));
        assert!(!plain.contains(",-- output"));
        assert!(!plain.contains("`--"));
    }

    #[test]
    fn renders_command_error_output_as_code_block() {
        let output = render_output_block("err exit 1", "not found\n");
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("── • err exit 1 "));
        assert!(plain.contains("not found"));
        assert!(!plain.contains(",-- err"));
        assert!(!plain.contains("`--"));
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

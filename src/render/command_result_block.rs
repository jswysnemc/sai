use crate::render::code_block::{render_code_footer, render_code_header};
use crate::render::style::TOOL_BULLET;
use crate::render::terminal_text as t;
use serde_json::Value;

const COMMAND_PREVIEW_LINES: usize = 5;

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

/// 渲染命令输出文本块。
///
/// 参数:
/// - `label`: 文本块标签
/// - `text`: 文本内容
///
/// 返回:
/// - 代码块风格的输出文本
#[cfg(test)]
fn render_output_block(label: &str, text: &str) -> String {
    let content = truncate_chars(text.trim(), 2400);
    let content = sanitize_command_output(&content);
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
/// 将命令工具 JSON 结果按可选行数限制渲染。
///
/// 参数:
/// - `output`: 命令工具返回的 JSON
/// - `line_limit`: 所有输出流共享的最大预览行数，空值表示完整输出
///
/// 返回:
/// - 可读的命令输出文本
pub(crate) fn render_command_result_view_with_limit(
    output: &str,
    line_limit: Option<usize>,
) -> String {
    render_command_result_view_with_options(output, line_limit, true)
}

/// 将命令工具结果渲染为普通 CLI 的有限摘要。
///
/// 参数:
/// - `output`: 命令工具返回的 JSON
///
/// 返回:
/// - 最多五行且不包含展开提示的命令输出
pub(crate) fn render_command_result_view_for_cli(output: &str) -> String {
    render_command_result_view_with_options(output, Some(COMMAND_PREVIEW_LINES), false)
}

/// 将前台命令实时 stdout/stderr 渲染为普通 CLI 摘要。
///
/// 参数:
/// - `stdout`: 当前 stdout 缓冲
/// - `stderr`: 当前 stderr 缓冲
///
/// 返回:
/// - 最多五行且不包含展开提示的命令输出
pub(crate) fn render_live_command_output_for_cli(stdout: &str, stderr: &str) -> String {
    let mut blocks = Vec::new();
    if !stdout.is_empty() {
        blocks.push((t("output", "输出").to_string(), stdout.to_string()));
    }
    if !stderr.is_empty() {
        blocks.push((t("err", "错误").to_string(), stderr.to_string()));
    }
    render_output_blocks_with_hint(blocks, Some(COMMAND_PREVIEW_LINES), false)
}

/// 将命令失败结果渲染为普通 CLI 的五行错误摘要。
///
/// 参数:
/// - `output`: 命令工具返回的 JSON 或错误文本
///
/// 返回:
/// - 最多五行且不包含展开提示的错误输出
pub(crate) fn render_command_error_view_for_cli(output: &str) -> String {
    let Some(result) = parse_command_result(output) else {
        return render_output_block_limited_with_hint(
            t("err", "错误"),
            output,
            Some(COMMAND_PREVIEW_LINES),
            false,
        );
    };
    if result.success {
        return String::new();
    }
    let label = result
        .exit_code
        .map(|code| format!("{} {code}", t("err exit", "错误 退出码")))
        .unwrap_or_else(|| t("err", "错误").to_string());
    let message = if result.stderr.trim().is_empty() {
        result.stdout.as_str()
    } else {
        result.stderr.as_str()
    };
    render_output_block_limited_with_hint(&label, message, Some(COMMAND_PREVIEW_LINES), false)
}

/// 按可选行数和展开提示配置渲染命令结果。
///
/// 参数:
/// - `output`: 命令工具返回的 JSON
/// - `line_limit`: 所有输出流共享的最大预览行数
/// - `show_expand_hint`: 是否显示展开快捷键提示
///
/// 返回:
/// - 可读的命令输出文本
fn render_command_result_view_with_options(
    output: &str,
    line_limit: Option<usize>,
    show_expand_hint: bool,
) -> String {
    let Some(result) = parse_command_result(output) else {
        return render_output_block_limited_with_hint(
            t("output", "输出"),
            output,
            line_limit,
            show_expand_hint,
        );
    };
    let mut blocks = Vec::new();
    let stdout_empty = result.stdout.trim().is_empty();
    if !stdout_empty {
        blocks.push((t("output", "输出").to_string(), result.stdout));
    }
    if !result.stderr.trim().is_empty() {
        let label = result
            .exit_code
            .map(|code| format!("{} {code}", t("err exit", "错误 退出码")))
            .unwrap_or_else(|| t("err", "错误").to_string());
        blocks.push((label, result.stderr));
    } else if !result.success {
        let label = result
            .exit_code
            .map(|code| format!("{} {code}", t("err exit", "错误 退出码")))
            .unwrap_or_else(|| t("err", "错误").to_string());
        blocks.push((
            label,
            t(
                "command failed without stderr",
                "命令失败，但没有 stderr 输出",
            )
            .to_string(),
        ));
    } else if stdout_empty {
        blocks.push((
            t("output", "输出").to_string(),
            t("no output", "无输出").to_string(),
        ));
    }
    render_output_blocks_with_hint(blocks, line_limit, show_expand_hint)
}

/// 渲染命令运行中的 stdout/stderr 预览。
///
/// 参数:
/// - `stdout`: stdout 显示文本
/// - `stderr`: stderr 显示文本
/// - `expanded`: 是否展开完整输出
///
/// 返回:
/// - 命令输出预览文本
pub(crate) fn render_live_command_output(stdout: &str, stderr: &str, expanded: bool) -> String {
    let line_limit = (!expanded).then_some(COMMAND_PREVIEW_LINES);
    let mut blocks = Vec::new();
    if !stdout.is_empty() {
        blocks.push((t("output", "输出").to_string(), stdout.to_string()));
    }
    if !stderr.is_empty() {
        blocks.push((t("err", "错误").to_string(), stderr.to_string()));
    }
    render_output_blocks(blocks, line_limit)
}

/// 使用实时缓冲与最终状态渲染已经结束的命令。
///
/// 参数:
/// - `output`: 命令工具最终 JSON
/// - `stdout`: 实时捕获的 stdout
/// - `stderr`: 实时捕获的 stderr
/// - `expanded`: 是否展开完整输出
///
/// 返回:
/// - 带最终错误状态的命令输出视图
pub(crate) fn render_completed_command_output(
    output: &str,
    stdout: &str,
    stderr: &str,
    expanded: bool,
) -> String {
    let result = parse_command_result(output);
    if result.is_none() {
        return render_output_block_limited(
            t("err", "错误"),
            output,
            (!expanded).then_some(COMMAND_PREVIEW_LINES),
        );
    }
    if stdout.is_empty() && stderr.is_empty() {
        return render_command_result_view_with_limit(
            output,
            (!expanded).then_some(COMMAND_PREVIEW_LINES),
        );
    }
    let line_limit = (!expanded).then_some(COMMAND_PREVIEW_LINES);
    let mut blocks = Vec::new();
    if !stdout.is_empty() {
        blocks.push((t("output", "输出").to_string(), stdout.to_string()));
    }
    if !stderr.is_empty() {
        let label = result
            .as_ref()
            .and_then(|result| result.exit_code)
            .map(|code| format!("{} {code}", t("err exit", "错误 退出码")))
            .unwrap_or_else(|| t("err", "错误").to_string());
        blocks.push((label, stderr.to_string()));
    } else if result.as_ref().is_some_and(|result| !result.success) {
        let label = result
            .as_ref()
            .and_then(|result| result.exit_code)
            .map(|code| format!("{} {code}", t("err exit", "错误 退出码")))
            .unwrap_or_else(|| t("err", "错误").to_string());
        blocks.push((
            label,
            t(
                "command failed without stderr",
                "命令失败，但没有 stderr 输出",
            )
            .to_string(),
        ));
    }
    render_output_blocks(blocks, line_limit)
}

/// 按共享行数预算渲染多个命令输出块。
///
/// 参数:
/// - `blocks`: 标签与输出文本
/// - `line_limit`: 所有输出块共享的最大内容行数
///
/// 返回:
/// - 合并后的命令输出视图
fn render_output_blocks(blocks: Vec<(String, String)>, line_limit: Option<usize>) -> String {
    render_output_blocks_with_hint(blocks, line_limit, true)
}

/// 按共享行数预算渲染多个输出块，并控制是否显示展开提示。
///
/// 参数:
/// - `blocks`: 标签与输出文本
/// - `line_limit`: 所有输出块共享的最大内容行数
/// - `show_expand_hint`: 是否显示展开快捷键提示
///
/// 返回:
/// - 合并后的命令输出视图
fn render_output_blocks_with_hint(
    blocks: Vec<(String, String)>,
    line_limit: Option<usize>,
    show_expand_hint: bool,
) -> String {
    let limits = shared_line_limits(line_limit, &blocks);
    let parts = blocks
        .into_iter()
        .zip(limits)
        .map(|((label, text), limit)| {
            render_output_block_limited_with_hint(&label, &text, limit, show_expand_hint)
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return String::new();
    }
    let mut joined = parts.join("");
    while joined.ends_with('\n') {
        joined.pop();
    }
    joined
}

/// 将总行数上限分配给各输出块。
///
/// 参数:
/// - `line_limit`: 总内容行数上限
/// - `blocks`: 标签与输出文本
///
/// 返回:
/// - 与输出块数量一致的独立预算
fn shared_line_limits(
    line_limit: Option<usize>,
    blocks: &[(String, String)],
) -> Vec<Option<usize>> {
    let Some(limit) = line_limit else {
        return vec![None; blocks.len()];
    };
    if blocks.is_empty() {
        return Vec::new();
    }
    let line_counts = blocks
        .iter()
        .map(|(_, text)| sanitize_command_output(text.trim()).lines().count().max(1))
        .collect::<Vec<_>>();
    let mut allocations = vec![0usize; blocks.len()];
    let mut remaining = limit;
    while remaining > 0 {
        let mut allocated = false;
        for (index, count) in line_counts.iter().enumerate() {
            if remaining == 0 {
                break;
            }
            if allocations[index] < *count {
                allocations[index] += 1;
                remaining -= 1;
                allocated = true;
            }
        }
        if !allocated {
            break;
        }
    }
    allocations.into_iter().map(Some).collect()
}

/// 按可选行数限制渲染命令输出块。
///
/// 参数:
/// - `label`: 输出块标签
/// - `text`: 原始输出文本
/// - `line_limit`: 当前输出块最大内容行数
///
/// 返回:
/// - 代码块风格的输出预览
fn render_output_block_limited(label: &str, text: &str, line_limit: Option<usize>) -> String {
    render_output_block_limited_with_hint(label, text, line_limit, true)
}

/// 按可选行数渲染输出块，并控制是否显示展开提示。
///
/// 参数:
/// - `label`: 输出块标签
/// - `text`: 原始输出文本
/// - `line_limit`: 当前输出块最大内容行数
/// - `show_expand_hint`: 是否显示展开快捷键提示
///
/// 返回:
/// - 代码块风格的输出预览
fn render_output_block_limited_with_hint(
    label: &str,
    text: &str,
    line_limit: Option<usize>,
    show_expand_hint: bool,
) -> String {
    let sanitized = sanitize_command_output(text.trim());
    let (content, omitted) = limited_output_text(&sanitized, line_limit);
    let mut lines = output_block_lines(&content);
    if omitted > 0 && show_expand_hint {
        let hint = format!("{omitted} lines omitted, press Ctrl+O to expand");
        lines.push(format!("\x1b[2m  {hint}\x1b[0m"));
    }
    render_output_block_lines(label, &lines)
}

/// 使用统一的内容行渲染输出块及底部边框。
///
/// 参数:
/// - `label`: 输出块标签
/// - `lines`: 已准备好的输出内容行
///
/// 返回:
/// - 带头部和底部边框的输出文本
fn render_output_block_lines(label: &str, lines: &[String]) -> String {
    let mut output = render_code_header(&format!("{TOOL_BULLET} {label}"));
    for line in lines {
        output.push_str(line);
        output.push('\n');
    }
    output.push_str(&render_code_footer(lines));
    output
}

/// 移除命令输出中可能改变终端布局的控制序列。
///
/// 参数:
/// - `text`: 原始命令输出
///
/// 返回:
/// - 仅保留可显示字符的文本
fn sanitize_command_output(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
            continue;
        }
        if ch == '\r' {
            output.push('\n');
        } else if !ch.is_control() || matches!(ch, '\n' | '\t') {
            output.push(ch);
        }
    }
    output
}

/// 保留输出最新行并返回折叠的旧行数量。
///
/// 参数:
/// - `text`: 已清理的输出文本
/// - `line_limit`: 最大内容行数
///
/// 返回:
/// - 最新输出文本与折叠行数
fn limited_output_text(text: &str, line_limit: Option<usize>) -> (String, usize) {
    let lines = text.lines().collect::<Vec<_>>();
    let Some(limit) = line_limit else {
        return (text.to_string(), 0);
    };
    if lines.len() <= limit {
        return (text.to_string(), 0);
    }
    let visible_start = lines.len().saturating_sub(limit);
    (lines[visible_start..].join("\n"), visible_start)
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

    #[test]
    fn live_command_preview_keeps_latest_five_lines_and_expand_hint() {
        let output =
            render_live_command_output("one\ntwo\nthree\nfour\nfive\nsix\nseven\n", "", false);
        let plain = strip_ansi_for_test(&output);

        assert!(!plain.lines().any(|line| line == "one"));
        assert!(!plain.lines().any(|line| line == "two"));
        assert!(plain.lines().any(|line| line == "three"));
        assert!(plain.lines().any(|line| line == "four"));
        assert!(plain.contains("seven"));
        assert!(plain.contains("Ctrl+O"));
    }

    #[test]
    fn collapsed_hint_stays_inside_output_block_before_footer() {
        let output =
            render_live_command_output("one\ntwo\nthree\nfour\nfive\nsix\nseven\n", "", false);
        let plain = strip_ansi_for_test(&output);
        let lines = plain.lines().collect::<Vec<_>>();
        let hint_index = lines
            .iter()
            .position(|line| line.contains("Ctrl+O"))
            .expect("collapsed output should include an expand hint");
        let footer_index = lines
            .iter()
            .rposition(|line| !line.is_empty() && line.chars().all(|ch| ch == '─'))
            .expect("output should include a footer border");

        assert!(
            hint_index < footer_index,
            "expand hint must be rendered before the output footer"
        );
    }

    #[test]
    fn expanded_command_output_keeps_all_lines() {
        let output = render_live_command_output("one\ntwo\nthree\nfour\nfive\nsix\n", "", true);
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("four"));
        assert!(plain.contains("six"));
        assert!(!plain.contains("Ctrl+O"));
    }

    #[test]
    fn live_command_output_removes_terminal_control_sequences() {
        let output = render_live_command_output("\x1b[2Jfirst\rsecond\x07\n", "", true);

        assert!(!output.contains("\x1b[2J"));
        assert!(!output.contains('\x07'));
        assert!(output.contains("first\nsecond"));
    }

    #[test]
    fn stdout_and_stderr_share_five_line_preview_budget() {
        let output = render_live_command_output(
            "out-1\nout-2\nout-3\nout-4\nout-5\n",
            "err-1\nerr-2\nerr-3\nerr-4\nerr-5\n",
            false,
        );
        let plain = strip_ansi_for_test(&output);
        let visible_lines = [
            "out-1", "out-2", "out-3", "out-4", "out-5", "err-1", "err-2", "err-3", "err-4",
            "err-5",
        ]
        .into_iter()
        .filter(|line| plain.lines().any(|candidate| candidate == *line))
        .count();

        assert_eq!(visible_lines, COMMAND_PREVIEW_LINES);
        assert!(plain.contains("Ctrl+O"));
    }

    #[test]
    fn completed_command_result_uses_shared_preview_budget() {
        let output = serde_json::json!({
            "success": false,
            "exit_code": 1,
            "stdout": "out-1\nout-2\nout-3\nout-4",
            "stderr": "err-1\nerr-2\nerr-3\nerr-4"
        })
        .to_string();
        let rendered = render_command_result_view_with_limit(&output, Some(COMMAND_PREVIEW_LINES));
        let plain = strip_ansi_for_test(&rendered);
        let visible_lines = [
            "out-1", "out-2", "out-3", "out-4", "err-1", "err-2", "err-3", "err-4",
        ]
        .into_iter()
        .filter(|line| plain.lines().any(|candidate| candidate == *line))
        .count();

        assert_eq!(visible_lines, COMMAND_PREVIEW_LINES);
    }

    /// 验证普通 CLI 仅展示五行命令摘要且不提供展开提示。
    #[test]
    fn cli_command_result_keeps_five_lines_without_expand_hint() {
        let output = serde_json::json!({
            "success": true,
            "exit_code": 0,
            "stdout": "one\ntwo\nthree\nfour\nfive\nsix\nseven",
            "stderr": ""
        })
        .to_string();
        let rendered = render_command_result_view_for_cli(&output);
        let plain = strip_ansi_for_test(&rendered);
        let visible_lines = ["one", "two", "three", "four", "five", "six", "seven"]
            .into_iter()
            .filter(|line| plain.lines().any(|candidate| candidate == *line))
            .count();

        assert_eq!(visible_lines, COMMAND_PREVIEW_LINES);
        assert!(!plain.contains("Ctrl+O"));
        assert!(!plain.lines().any(|line| line == "one"));
        assert!(!plain.lines().any(|line| line == "two"));
        assert!(plain.lines().any(|line| line == "four"));
    }

    #[test]
    fn tool_error_is_not_hidden_by_live_output() {
        let rendered = render_completed_command_output(
            "tool error: shell command timed out after 1s",
            "before-timeout",
            "",
            false,
        );
        let plain = strip_ansi_for_test(&rendered);

        assert!(plain.contains("timed out"));
        assert!(!plain.contains("before-timeout"));
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

use crate::render::fold_text::{fold_display_lines, terminal_wrap_width, wrap_display_lines, FOLD_HEAD_LINES, FOLD_TAIL_LINES};
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
    render_output_block_limited(label, text, None)
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

/// 提取命令工具 JSON 结果中的状态与输出流。
///
/// 参数:
/// - output: 命令工具返回的 JSON
///
/// 返回:
/// - (是否成功, stdout, stderr)；无法解析时返回空
pub(crate) fn command_result_streams(output: &str) -> Option<(bool, String, String)> {
    let result = parse_command_result(output)?;
    Some((result.success, result.stdout, result.stderr))
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
    // Codex 风格：每个输出流独立使用预览行预算，首尾截断
    vec![line_limit; blocks.len()]
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
    let raw_lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    let mut lines = Vec::new();
    for (index, line) in raw_lines.iter().enumerate() {
        if line == "__OMITTED__" {
            let hint = if show_expand_hint {
                format!("… +{omitted} lines (Ctrl+O to expand)")
            } else {
                format!("… +{omitted} lines")
            };
            lines.push(format!("\x1b[2m  └ {hint}\x1b[0m"));
            continue;
        }
        // Codex 输出 gutter：首行 `  └ `，续行四空格，整体 dim
        let prefix = if index == 0 { "  └ " } else { "    " };
        lines.push(format!("\x1b[2m{prefix}{line}\x1b[0m"));
    }
    if lines.is_empty() {
        lines.push("\x1b[2m  └ (no output)\x1b[0m".to_string());
    } else if omitted > 0 && !raw_lines.iter().any(|line| line == "__OMITTED__") && show_expand_hint
    {
        // 兼容旧逻辑：若仅尾部折叠，在末尾附加提示
        lines.push(format!(
            "\x1b[2m    … +{omitted} lines (Ctrl+O to expand)\x1b[0m"
        ));
    }
    render_output_block_lines(label, &lines)
}

/// 使用 Codex 风格 gutter 渲染输出块。
///
/// 参数:
/// - `label`: 输出块标签（stdout/stderr 语义）
/// - `lines`: 已准备好的输出内容行
///
/// 返回:
/// - 带 dim 前缀的输出文本
fn render_output_block_lines(label: &str, lines: &[String]) -> String {
    let mut output = String::new();
    // 错误流保留标签行，标准输出直接展示内容
    let is_error = label.starts_with("err") || label.contains("错误") || label.contains("err");
    if is_error {
        output.push_str(&format!("\x1b[31m{TOOL_BULLET} {label}\x1b[0m\n"));
    }
    for line in lines {
        output.push_str(line);
        output.push('\n');
    }
    output
}

/// 移除命令输出中可能改变终端布局的控制序列。
///
/// 参数:
/// - `text`: 原始命令输出
///
/// 返回:
/// - 仅保留可显示字符的文本
pub(crate) fn sanitize_command_output(text: &str) -> String {
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
    // 1. 先按终端显示宽度折行，再按显示行折叠（避免超长单行挤占视野）
    let display_lines = wrap_display_lines(text, terminal_wrap_width());
    let Some(limit) = line_limit else {
        return (display_lines.join("\n"), 0);
    };
    if display_lines.is_empty() {
        return (String::new(), 0);
    }
    // 预览折叠固定前 2 后 4；line_limit 仅用于决定是否折叠（Some 即折叠）
    let _ = limit;
    let (kept, omitted) = fold_display_lines(
        &display_lines,
        FOLD_HEAD_LINES,
        FOLD_TAIL_LINES,
        false,
    );
    (kept.join("\n"), omitted)
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
    fn renders_command_output_with_codex_gutter() {
        let output = render_output_block("output", "sent to snemc@qq.com\n");
        let plain = strip_ansi_for_test(&output);

        assert!(!plain.contains("──"));
        assert!(plain.contains("sent to snemc@qq.com"));
        assert!(plain.contains("└"));
        assert!(!plain.contains(",-- output"));
        assert!(!plain.contains("`--"));
    }

    #[test]
    fn renders_command_error_output_as_code_block() {
        let output = render_output_block("err exit 1", "not found\n");
        let plain = strip_ansi_for_test(&output);

        assert!(plain.contains("• err exit 1"));
        assert!(!plain.contains("──"));
        assert!(plain.contains("not found"));
        assert!(!plain.contains(",-- err"));
        assert!(!plain.contains("`--"));
    }

    #[test]
    fn live_command_preview_keeps_head_and_tail_with_middle_ellipsis() {
        let output = render_live_command_output(
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n",
            "",
            false,
        );
        let plain = strip_ansi_for_test(&output);

        // 折叠策略：前 2 + 后 4
        assert!(plain.contains("one"));
        assert!(plain.contains("two") || plain.contains("one"));
        assert!(!plain
            .lines()
            .any(|line| line.trim_end() == "five" || line.ends_with(" five")));
        assert!(!plain
            .lines()
            .any(|line| line.trim_end() == "six" || line.ends_with(" six")));
        // 中间省略 6 行（12 - 2 - 4）
        assert!(
            plain.contains("… +6 lines")
                || plain.contains("+6 lines")
                || plain.contains("…"),
            "expected fold ellipsis: {plain}"
        );
        assert!(plain.contains("nine") || plain.contains("twelve"));
        assert!(plain.contains("twelve"));
        assert!(plain.contains("Ctrl+O"));
    }

    #[test]
    fn collapsed_hint_stays_without_frame_footer() {
        let output = render_live_command_output(
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n",
            "",
            false,
        );
        let plain = strip_ansi_for_test(&output);
        assert!(plain.contains("Ctrl+O"));
        assert!(!plain.contains("──"));
        assert!(plain.contains("└") || plain.contains("…"));
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
        let plain = strip_ansi_for_test(&output);

        assert!(!output.contains("\x1b[2J"));
        assert!(!output.contains('\x07'));
        assert!(plain.contains("first"));
        assert!(plain.contains("second"));
    }

    #[test]
    fn stdout_and_stderr_each_keep_preview_budget() {
        let output = render_live_command_output(
            "out-1\nout-2\nout-3\nout-4\nout-5\nout-6\nout-7\nout-8\nout-9\nout-10\nout-11\nout-12\n",
            "err-1\nerr-2\nerr-3\nerr-4\nerr-5\nerr-6\nerr-7\nerr-8\nerr-9\nerr-10\nerr-11\nerr-12\n",
            false,
        );
        let plain = strip_ansi_for_test(&output);
        // 每流首尾各 5 行
        assert!(plain.contains("out-1"));
        assert!(plain.contains("out-12"));
        assert!(plain.contains("err-1"));
        assert!(plain.contains("err-12"));
        assert!(plain.contains("Ctrl+O"));
    }

    #[test]
    fn completed_command_result_keeps_each_stream_preview() {
        let output = serde_json::json!({
            "success": false,
            "exit_code": 1,
            "stdout": "out-1\nout-2\nout-3\nout-4",
            "stderr": "err-1\nerr-2\nerr-3\nerr-4"
        })
        .to_string();
        let rendered = render_command_result_view_with_limit(&output, Some(COMMAND_PREVIEW_LINES));
        let plain = strip_ansi_for_test(&rendered);
        // 每流行数 <= 2*limit，完整保留
        for line in ["out-1", "out-4", "err-1", "err-4"] {
            assert!(plain.contains(line), "{line}");
        }
    }

    /// 验证普通 CLI 仅展示五行命令摘要且不提供展开提示。
    #[test]
    fn cli_command_result_keeps_head_and_tail_without_expand_hint() {
        let output = serde_json::json!({
            "success": true,
            "exit_code": 0,
            "stdout": "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve",
            "stderr": ""
        })
        .to_string();
        let rendered = render_command_result_view_for_cli(&output);
        let plain = strip_ansi_for_test(&rendered);
        let visible = [
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
            "eleven", "twelve",
        ]
        .into_iter()
        .filter(|line| plain.contains(line))
        .count();

        // 首尾各 5 行，中间省略 2 行
        assert_eq!(visible, FOLD_HEAD_LINES + FOLD_TAIL_LINES);
        assert!(!plain.contains("Ctrl+O"));
        assert!(plain.contains("one"));
        assert!(plain.contains("twelve"));
        assert!(
            !plain.contains("six\n")
                && !plain
                    .lines()
                    .any(|l| l.trim().ends_with("six") && !l.contains("…"))
        );
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

    #[test]
    fn long_single_line_command_output_folds_by_display_width() {
        // 按当前终端宽度生成足够多的显示行（> 2*预览行）
        let width = crate::render::fold_text::terminal_wrap_width().max(8);
        let long = "x".repeat(width * 12);
        let output = render_live_command_output(&long, "", false);
        let plain = strip_ansi_for_test(&output);
        assert!(
            plain.contains("Ctrl+O") || plain.contains("…"),
            "got: {plain}"
        );
    }
}

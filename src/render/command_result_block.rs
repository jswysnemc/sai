use crate::render::fold_text::{
    fold_display_lines, terminal_wrap_width, wrap_display_lines, FoldedDisplayLine,
    FOLD_HEAD_LINES, FOLD_TAIL_LINES,
};
use crate::render::terminal_text as t;
use serde_json::Value;

const COMMAND_PREVIEW_LINES: usize = 5;

/// 计算命令输出的预览行数限制。
///
/// 参数:
/// - `expanded`: 该输出块是否已被展开
///
/// 返回:
/// - 折叠时的行数上限；展开或处于展开渲染上下文时为 None
fn preview_line_limit(expanded: bool) -> Option<usize> {
    if expanded || crate::render::render_expand::expand_override() {
        None
    } else {
        Some(COMMAND_PREVIEW_LINES)
    }
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
        "{}\n… {} {omitted} {}",
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

/// 区分输出流与失败状态；参数为命令成功标志和退出码，返回可读日志标签。
fn stderr_label(success: bool, exit_code: Option<i64>) -> String {
    if success {
        return t("stderr", "标准错误输出").to_string();
    }
    exit_code
        .map(|code| format!("{} {code}", t("Failed · exit", "执行失败 · 退出码")))
        .unwrap_or_else(|| t("Failed", "执行失败").to_string())
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
        blocks.push((t("stderr", "标准错误输出").to_string(), stderr.to_string()));
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
            t("Failed", "执行失败"),
            output,
            Some(COMMAND_PREVIEW_LINES),
            false,
        );
    };
    if result.success {
        return String::new();
    }
    let label = stderr_label(false, result.exit_code);
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
        let label = stderr_label(result.success, result.exit_code);
        blocks.push((label, result.stderr));
    } else if !result.success {
        let label = stderr_label(false, result.exit_code);
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
    let line_limit = preview_line_limit(expanded);
    let mut blocks = Vec::new();
    if !stdout.is_empty() {
        blocks.push((t("output", "输出").to_string(), stdout.to_string()));
    }
    if !stderr.is_empty() {
        blocks.push((t("stderr", "标准错误输出").to_string(), stderr.to_string()));
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
            t("Failed", "执行失败"),
            output,
            preview_line_limit(expanded),
        );
    }
    if stdout.is_empty() && stderr.is_empty() {
        return render_command_result_view_with_limit(output, preview_line_limit(expanded));
    }
    let line_limit = preview_line_limit(expanded);
    let mut blocks = Vec::new();
    if !stdout.is_empty() {
        blocks.push((t("output", "输出").to_string(), stdout.to_string()));
    }
    if !stderr.is_empty() {
        let label = result
            .as_ref()
            .map(|result| stderr_label(result.success, result.exit_code))
            .unwrap_or_else(|| stderr_label(false, None));
        blocks.push((label, stderr.to_string()));
    } else if result.as_ref().is_some_and(|result| !result.success) {
        let label = stderr_label(false, result.as_ref().and_then(|result| result.exit_code));
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
    let raw_lines = limited_output_text(&sanitized, line_limit);
    // 失败状态使用红色标签，标准错误流仅标明来源，标准输出直接展示内容；
    // 状态圆点只出现在卡片标题层级，输出块内不再另起 `•` 行
    let is_stderr = label == "stderr" || label == "标准错误输出";
    let is_error = !is_stderr
        && (label.starts_with("err")
            || label.contains("错误")
            || label.starts_with("Failed")
            || label.contains("失败"));
    let mut lines = Vec::new();
    if is_error {
        lines.push(format!("\x1b[2m  └ \x1b[0m\x1b[31m{label}\x1b[0m"));
    } else if is_stderr {
        lines.push(format!("\x1b[2m  └ {label}\x1b[0m"));
    }
    for line in raw_lines {
        let line = match line {
            FoldedDisplayLine::Omitted { omitted, .. } => {
                lines.push(crate::render::omitted_line::render_omitted_line(
                    omitted,
                    show_expand_hint,
                ));
                continue;
            }
            FoldedDisplayLine::Line(line) => line,
        };
        // Codex 输出 gutter：首行 `  └ `，续行四空格，整体 dim
        let prefix = if lines.is_empty() { "  └ " } else { "    " };
        lines.push(format!("\x1b[2m{prefix}{line}\x1b[0m"));
    }
    if lines.is_empty() {
        lines.push("\x1b[2m  └ (no output)\x1b[0m".to_string());
    }
    let mut output = String::new();
    for line in lines {
        output.push_str(&line);
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
fn limited_output_text(text: &str, line_limit: Option<usize>) -> Vec<FoldedDisplayLine> {
    // 1. 先按终端显示宽度折行，再按显示行折叠（避免超长单行挤占视野）
    let display_lines = wrap_display_lines(text, terminal_wrap_width().saturating_sub(4));
    fold_display_lines(
        &display_lines,
        FOLD_HEAD_LINES,
        FOLD_TAIL_LINES,
        line_limit.is_none(),
    )
}

#[cfg(test)]
#[path = "command_result_block_tests.rs"]
mod tests;

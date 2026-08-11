use super::colors::{
    style_added_count, style_added_line, style_context_line, style_removed_count,
    style_removed_line,
};
use super::model::preview_from_arguments;
use crate::render::content_indent::{
    clear_right_margin, indent_diff_for_cli, indent_diff_for_transcript, DIFF_BLOCK_INSET,
};
use crate::render::style::TOOL_BULLET;
use crate::tools::file_change_model::{AppliedPatch, FileChange, LineChange, LineChangeKind};
use anyhow::Result;
use std::io::{self, Write};
use std::path::Path;

/// 写入编辑文件 diff 视图。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `arguments`: `edit_file` / `write_file` / `str_replace` 工具参数
///
/// 返回:
/// - 是否成功渲染 diff 视图
pub(crate) fn write_edit_file_diff_block(stdout: &mut io::Stdout, arguments: &str) -> Result<bool> {
    let Some(diff) = render_edit_file_diff(arguments) else {
        return Ok(false);
    };
    write!(stdout, "{diff}")?;
    Ok(true)
}

/// 渲染编辑文件 diff 视图。
///
/// 参数:
/// - `arguments`: `edit_file` / `write_file` / `str_replace` 工具参数
///
/// 返回:
/// - Codex 风格 diff 文本
pub(crate) fn render_edit_file_diff(arguments: &str) -> Option<String> {
    let preview = preview_from_arguments(arguments).ok()?;
    let rendered = render_patch_preview(&preview);
    // CLI 侧此前不折行，交给终端硬换行：续行落在第 0 列且不带背景色，
    // 增删行的矩形色块因此在每个续行行首缺一个口子。改走与 TUI 相同的
    // 折行入口，续行先恢复背景再补正文缩进
    let wrapped = wrap_cli_diff(&rendered);
    let diff = indent_diff_for_cli(&wrapped);
    Some(inset_cli_diff_background(&diff))
}

/// 【终端】【Diff 换行】按终端可用列数折行 CLI diff。
///
/// 折行宽度需扣除左右两侧的块级内收：左侧由 `indent_diff_for_cli` 补，
/// 右侧留给背景边距，两者不扣就会把色块顶出终端。
///
/// 参数:
/// - `rendered`: 未缩进的 diff 文本块
///
/// 返回:
/// - 已按正文净宽折行、续行带背景与缩进的文本块
fn wrap_cli_diff(rendered: &str) -> String {
    if rendered.is_empty() {
        return String::new();
    }
    let content_width = crate::render::fold_text::terminal_wrap_width()
        .saturating_sub(DIFF_BLOCK_INSET.saturating_mul(2))
        .max(1);
    let body_column = diff_body_start_column(rendered);
    let lines = crate::render::transcript::AnsiLine::wrap_block_with_continuation_indent(
        rendered,
        content_width,
        body_column.max(crate::render::content_indent::DIFF_NESTED_INDENT),
    );
    lines
        .iter()
        .map(|line| line.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 渲染供 TUI transcript 使用的编辑文件 diff 视图。
///
/// TUI 会在窗口层添加正文基线，本入口只增加 diff 的内部层级。
///
/// 参数:
/// - `arguments`: `edit_file` / `write_file` / `str_replace` 工具参数
///
/// 返回:
/// - 相对正文内收一列的 Codex 风格 diff 文本
pub(crate) fn render_edit_file_diff_for_transcript(arguments: &str) -> Option<String> {
    let preview = preview_from_arguments(arguments).ok()?;
    Some(indent_diff_for_transcript(&render_patch_preview(&preview)))
}

/// 【终端】【Diff 换行】推导 diff 正文相对行首的起始列。
///
/// 每个 diff 正文行的格式是 `行号 标记  内容`，行号列宽在同一份 diff 内一致。
/// 长行折行时续行需要缩进到该列，否则会顶到最左侧、与行号列错位。
///
/// 参数:
/// - `rendered`: 已渲染的 diff 文本块
///
/// 返回:
/// - 正文起始列；无法识别时返回 0
pub(crate) fn diff_body_start_column(rendered: &str) -> usize {
    rendered
        .lines()
        .filter_map(body_start_column_of_line)
        .next()
        .unwrap_or(0)
}

/// 【终端】【Diff 换行】解析单行的正文起始列。
///
/// 参数:
/// - `line`: 带 ANSI 样式的 diff 行
///
/// 返回:
/// - 该行正文起始列；不是正文行时返回 None
fn body_start_column_of_line(line: &str) -> Option<usize> {
    // 只统计可见字符：行号右对齐填充、标记与两列间隔都在正文之前
    let visible = strip_ansi_sequences(line);
    // 标题行以 TOOL_BULLET 开头，其中的增删计数也含数字，必须先排除
    if visible.contains(TOOL_BULLET) {
        return None;
    }
    let digits_end = visible.find(|ch: char| ch.is_ascii_digit())?;
    // 行号之前只允许右对齐填充空格
    if !visible[..digits_end].chars().all(|ch| ch == ' ') {
        return None;
    }
    let rest = &visible[digits_end..];
    let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    let after_digits = &rest[digit_count..];
    // 行号后依次是一列空格、一列标记、两列间隔
    let marker_offset = after_digits.strip_prefix(' ')?;
    let marker = marker_offset.chars().next()?;
    if !matches!(marker, '+' | '-' | ' ') {
        return None;
    }
    Some(digits_end + digit_count + 4)
}

/// 【终端】【Diff 换行】移除 ANSI 控制序列。
///
/// 参数:
/// - `text`: 带样式的终端行
///
/// 返回:
/// - 仅含可见字符的文本
fn strip_ansi_sequences(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < text.len() {
        if text[index..].starts_with('\x1b') {
            // 交由统一的转义扫描定位序列结束，自行判断终止符会把 `[` 误判为结尾
            index = crate::render::terminal_image::escape_sequence_end(text, index).max(index + 1);
            continue;
        }
        let ch = text[index..].chars().next().unwrap_or_default();
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

/// 清除 CLI diff 色块右侧边距。
///
/// 参数:
/// - `diff`: 已添加 CLI 左侧缩进的 diff 文本
///
/// 返回:
/// - 色块右侧保留对称边距的 ANSI 文本
fn inset_cli_diff_background(diff: &str) -> String {
    let clear = clear_right_margin(DIFF_BLOCK_INSET);
    diff.replace("\x1b[K\x1b[0m", &format!("\x1b[K\x1b[0m{clear}"))
}

/// 渲染 patch 预览。
///
/// 参数:
/// - `preview`: 文件变更预览
///
/// 返回:
/// - 终端 diff 文本
fn render_patch_preview(preview: &AppliedPatch) -> String {
    let mut output = String::new();
    output.push_str(&render_summary_header(preview));
    for (index, change) in preview.changes.iter().enumerate() {
        if preview.changes.len() > 1 {
            output.push_str(&render_file_header(change));
        }
        output.push_str(&render_file_change(change));
        if index + 1 < preview.changes.len() {
            output.push('\n');
        }
    }
    output
}

/// 渲染总摘要标题。
///
/// 参数:
/// - `preview`: 文件变更预览
///
/// 返回:
/// - 标题行
fn render_summary_header(preview: &AppliedPatch) -> String {
    if let [change] = preview.changes.as_slice() {
        return render_file_header(change);
    }
    let (added, removed) = total_line_counts(preview);
    let file_count = preview.changes.len();
    let noun = if file_count == 1 { "file" } else { "files" };
    format!(
        "{TOOL_BULLET} Edited {file_count} {noun} ({} {})\n",
        style_added_count(added),
        style_removed_count(removed)
    )
}

/// 渲染单文件标题。
///
/// 参数:
/// - `change`: 文件变更
///
/// 返回:
/// - 文件标题行
fn render_file_header(change: &FileChange) -> String {
    let (added, removed) = change.line_counts();
    let path = display_change_path(change);
    format!(
        "{TOOL_BULLET} {} {} ({} {})\n",
        change.action_label(),
        path,
        style_added_count(added),
        style_removed_count(removed)
    )
}

/// 渲染单文件 diff。
///
/// 参数:
/// - `change`: 文件变更
///
/// 返回:
/// - 文件 diff 文本
fn render_file_change(change: &FileChange) -> String {
    match change {
        FileChange::Add { path, content } => render_added_file(path, content),
        FileChange::Update { path, lines, .. } => render_update_lines(path, lines),
    }
}

/// 渲染新增文件。
///
/// 参数:
/// - `path`: 文件路径
/// - `content`: 新文件内容
///
/// 返回:
/// - diff 文本
fn render_added_file(path: &Path, content: &str) -> String {
    let width = content.lines().count().max(1).to_string().len().max(3);
    let mut output = String::new();
    for (index, line) in content.lines().enumerate() {
        output.push_str(&style_added_line(
            path,
            &format!("{:>width$} +  {line}", index + 1),
        ));
        output.push('\n');
    }
    output
}

/// 渲染更新文件行。
///
/// 参数:
/// - `path`: 文件路径
/// - `lines`: diff 行
///
/// 返回:
/// - diff 文本
fn render_update_lines(path: &Path, lines: &[LineChange]) -> String {
    let width = max_line_number(lines).to_string().len().max(3);
    let mut output = String::new();
    for line in lines {
        let (number, marker) = match line.kind {
            LineChangeKind::Context => (line.old_line.or(line.new_line).unwrap_or_default(), " "),
            LineChangeKind::Add => (line.new_line.unwrap_or_default(), "+"),
            LineChangeKind::Delete => (line.old_line.unwrap_or_default(), "-"),
        };
        let text = format!("{number:>width$} {marker}  {}", line.text);
        let styled = match line.kind {
            LineChangeKind::Context => style_context_line(path, &text),
            LineChangeKind::Add => style_added_line(path, &text),
            LineChangeKind::Delete => style_removed_line(path, &text),
        };
        output.push_str(&styled);
        output.push('\n');
    }
    output
}

/// 统计最大行号。
///
/// 参数:
/// - `lines`: diff 行
///
/// 返回:
/// - 最大行号
fn max_line_number(lines: &[LineChange]) -> usize {
    lines
        .iter()
        .flat_map(|line| [line.old_line, line.new_line])
        .flatten()
        .max()
        .unwrap_or(1)
}

/// 统计总新增和删除行数。
///
/// 参数:
/// - `preview`: 文件变更预览
///
/// 返回:
/// - `(新增行数, 删除行数)`
fn total_line_counts(preview: &AppliedPatch) -> (usize, usize) {
    preview
        .changes
        .iter()
        .map(FileChange::line_counts)
        .fold((0, 0), |acc, item| (acc.0 + item.0, acc.1 + item.1))
}

/// 显示文件变更路径。
///
/// 参数:
/// - `change`: 文件变更
///
/// 返回:
/// - 展示路径文本
fn display_change_path(change: &FileChange) -> String {
    match change {
        FileChange::Update {
            path,
            move_path: Some(move_path),
            ..
        } => format!("{} -> {}", path.display(), move_path.display()),
        _ => change.path().display().to_string(),
    }
}

#[cfg(test)]
pub(super) fn render_for_test(arguments: &str) -> Option<String> {
    render_edit_file_diff(arguments)
}

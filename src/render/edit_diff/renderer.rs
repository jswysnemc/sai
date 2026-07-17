use super::colors::{
    style_added_count, style_added_line, style_context_line, style_removed_count,
    style_removed_line,
};
use super::model::preview_from_arguments;
use crate::render::style::TOOL_BULLET;
use crate::tools::edit_patch::{AppliedPatch, FileChange, LineChange, LineChangeKind};
use anyhow::Result;
use std::io::{self, Write};
use std::path::Path;

/// 写入编辑文件 diff 视图。
///
/// 参数:
/// - `stdout`: 标准输出句柄
/// - `arguments`: `edit_file` 工具参数
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
/// - `arguments`: `edit_file` 工具参数
///
/// 返回:
/// - Codex 风格 diff 文本
pub(crate) fn render_edit_file_diff(arguments: &str) -> Option<String> {
    let preview = preview_from_arguments(arguments).ok()?;
    Some(render_patch_preview(&preview))
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
        FileChange::Delete { path, content } => render_deleted_file(path, content),
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

/// 渲染删除文件。
///
/// 参数:
/// - `path`: 文件路径
/// - `content`: 旧文件内容
///
/// 返回:
/// - diff 文本
fn render_deleted_file(path: &Path, content: &str) -> String {
    let width = content.lines().count().max(1).to_string().len().max(3);
    let mut output = String::new();
    for (index, line) in content.lines().enumerate() {
        output.push_str(&style_removed_line(
            path,
            &format!("{:>width$} -  {line}", index + 1),
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

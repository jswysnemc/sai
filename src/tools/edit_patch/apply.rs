use super::model::{
    AppliedPatch, FileChange, LineChange, LineChangeKind, ParsedFileChange, PatchHunk, PatchLine,
};
use super::parser::parse_patch;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// 预览 patch 将产生的文件变更。
///
/// 参数:
/// - `patch`: Codex 风格 patch 文本
/// - `cwd`: 相对路径解析基准目录
///
/// 返回:
/// - 文件变更预览
pub(crate) fn preview_patch(patch: &str, cwd: &Path) -> Result<AppliedPatch> {
    let parsed = parse_patch(patch)?;
    let mut changes = Vec::new();
    for change in parsed.changes {
        changes.push(preview_change(change, cwd)?);
    }
    Ok(AppliedPatch { changes })
}

/// 应用 patch 并返回文件变更预览。
///
/// 参数:
/// - `patch`: Codex 风格 patch 文本
/// - `cwd`: 相对路径解析基准目录
///
/// 返回:
/// - 已应用的文件变更
pub(crate) fn apply_patch(patch: &str, cwd: &Path) -> Result<AppliedPatch> {
    let preview = preview_patch(patch, cwd)?;
    if preview.is_empty() {
        bail!("patch contains no changes")
    }
    for change in &preview.changes {
        apply_change(change)?;
    }
    Ok(preview)
}

/// 预览单个文件变更。
///
/// 参数:
/// - `change`: 已解析的文件变更
/// - `cwd`: 相对路径解析基准目录
///
/// 返回:
/// - 可渲染的文件变更
fn preview_change(change: ParsedFileChange, cwd: &Path) -> Result<FileChange> {
    match change {
        ParsedFileChange::Add { path, content } => {
            let path = resolve_path(cwd, &path);
            if path.exists() {
                bail!("Add File target already exists: {}", path.display())
            }
            Ok(FileChange::Add { path, content })
        }
        ParsedFileChange::Delete { path } => {
            let path = resolve_existing_file(cwd, &path)?;
            let content = std::fs::read_to_string(&path)?;
            Ok(FileChange::Delete { path, content })
        }
        ParsedFileChange::Update {
            path,
            move_path,
            hunks,
        } => {
            let path = resolve_existing_file(cwd, &path)?;
            let move_path = move_path.map(|target| resolve_path(cwd, &target));
            let old_content = std::fs::read_to_string(&path)?;
            let (new_content, lines) = apply_hunks_to_content(&old_content, &hunks)?;
            Ok(FileChange::Update {
                path,
                move_path,
                new_content,
                lines,
            })
        }
    }
}

/// 将 hunk 应用到文件内容。
///
/// 参数:
/// - `content`: 原始文件内容
/// - `hunks`: patch hunks
///
/// 返回:
/// - 新文件内容和可渲染 diff 行
fn apply_hunks_to_content(content: &str, hunks: &[PatchHunk]) -> Result<(String, Vec<LineChange>)> {
    if hunks.is_empty() {
        return Ok((content.to_string(), Vec::new()));
    }
    let had_trailing_newline = content.ends_with('\n');
    let mut lines = content.lines().map(str::to_string).collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut rendered = Vec::new();

    for hunk in hunks {
        let old_block = old_block_for_hunk(hunk);
        let new_block = new_block_for_hunk(hunk);
        let start = if old_block.is_empty() {
            cursor.min(lines.len())
        } else {
            find_subsequence(&lines, &old_block, cursor)
                .ok_or_else(|| anyhow::anyhow!("patch hunk did not match file content"))?
        };
        rendered.extend(render_hunk_lines(hunk, start));
        lines.splice(start..start + old_block.len(), new_block.clone());
        cursor = start + new_block.len();
    }

    let mut updated = lines.join("\n");
    if had_trailing_newline && !updated.is_empty() {
        updated.push('\n');
    }
    Ok((updated, rendered))
}

/// 提取 hunk 旧内容块。
///
/// 参数:
/// - `hunk`: patch hunk
///
/// 返回:
/// - 用于匹配原文件的行序列
fn old_block_for_hunk(hunk: &PatchHunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|line| match line {
            PatchLine::Context(text) | PatchLine::Delete(text) => Some(text.clone()),
            PatchLine::Add(_) => None,
        })
        .collect()
}

/// 提取 hunk 新内容块。
///
/// 参数:
/// - `hunk`: patch hunk
///
/// 返回:
/// - 替换后的行序列
fn new_block_for_hunk(hunk: &PatchHunk) -> Vec<String> {
    hunk.lines
        .iter()
        .filter_map(|line| match line {
            PatchLine::Context(text) | PatchLine::Add(text) => Some(text.clone()),
            PatchLine::Delete(_) => None,
        })
        .collect()
}

/// 生成 hunk 渲染行。
///
/// 参数:
/// - `hunk`: patch hunk
/// - `start`: hunk 在旧文件中的 0 起始下标
///
/// 返回:
/// - 可渲染 diff 行
fn render_hunk_lines(hunk: &PatchHunk, start: usize) -> Vec<LineChange> {
    let mut old_line = start + 1;
    let mut new_line = start + 1;
    let mut output = Vec::new();
    for line in &hunk.lines {
        match line {
            PatchLine::Context(text) => {
                output.push(LineChange {
                    kind: LineChangeKind::Context,
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                    text: text.clone(),
                });
                old_line += 1;
                new_line += 1;
            }
            PatchLine::Delete(text) => {
                output.push(LineChange {
                    kind: LineChangeKind::Delete,
                    old_line: Some(old_line),
                    new_line: None,
                    text: text.clone(),
                });
                old_line += 1;
            }
            PatchLine::Add(text) => {
                output.push(LineChange {
                    kind: LineChangeKind::Add,
                    old_line: None,
                    new_line: Some(new_line),
                    text: text.clone(),
                });
                new_line += 1;
            }
        }
    }
    output
}

/// 查找子序列。
///
/// 参数:
/// - `lines`: 原始行
/// - `needle`: 需要匹配的行
/// - `start`: 搜索起始下标
///
/// 返回:
/// - 匹配起始下标
fn find_subsequence(lines: &[String], needle: &[String], start: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(start.min(lines.len()));
    }
    if needle.len() > lines.len() {
        return None;
    }
    let last_start = lines.len() - needle.len();
    if start > last_start {
        return None;
    }
    (start..=last_start).find(|index| &lines[*index..*index + needle.len()] == needle)
}

/// 应用单个文件变更。
///
/// 参数:
/// - `change`: 文件变更
///
/// 返回:
/// - 写入是否成功
fn apply_change(change: &FileChange) -> Result<()> {
    match change {
        FileChange::Add { path, content } => write_text_file(path, content),
        FileChange::Delete { path, .. } => {
            std::fs::remove_file(path)?;
            Ok(())
        }
        FileChange::Update {
            path,
            move_path,
            new_content,
            ..
        } => {
            let target = move_path.as_ref().unwrap_or(path);
            write_text_file(target, new_content)?;
            if move_path.as_ref().is_some_and(|target| target != path) {
                std::fs::remove_file(path)?;
            }
            Ok(())
        }
    }
}

/// 原子写入 UTF-8 文本文件。
///
/// 参数:
/// - `path`: 目标路径
/// - `content`: 文件内容
///
/// 返回:
/// - 写入是否成功
fn write_text_file(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content.as_bytes())?;
    temp.persist(path)?;
    Ok(())
}

/// 解析已有普通文件路径。
///
/// 参数:
/// - `cwd`: 相对路径基准目录
/// - `path`: 输入路径
///
/// 返回:
/// - 可读普通文件路径
fn resolve_existing_file(cwd: &Path, path: &Path) -> Result<PathBuf> {
    let path = resolve_path(cwd, path);
    let canonical = path.canonicalize()?;
    if !canonical.is_file() {
        bail!("not a regular file: {}", path.display())
    }
    Ok(path)
}

/// 解析工具路径。
///
/// 参数:
/// - `cwd`: 相对路径基准目录
/// - `path`: 输入路径
///
/// 返回:
/// - 绝对或工作目录相对路径
fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_update_patch_with_context() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();

        apply_patch(
            "*** Begin Patch\n*** Update File: sample.txt\n@@\n one\n-two\n+TWO\n three\n*** End Patch",
            temp.path(),
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "one\nTWO\nthree\n");
    }

    #[test]
    fn applies_add_and_delete_patch() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("old.txt"), "old\n").unwrap();

        let preview = apply_patch(
            "*** Begin Patch\n*** Add File: new.txt\n+new\n*** Delete File: old.txt\n*** End Patch",
            temp.path(),
        )
        .unwrap();

        assert_eq!(preview.changes.len(), 2);
        assert_eq!(
            std::fs::read_to_string(temp.path().join("new.txt")).unwrap(),
            "new\n"
        );
        assert!(!temp.path().join("old.txt").exists());
    }
}

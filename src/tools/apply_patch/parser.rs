//! 严格解析补丁边界；拒绝残缺输入，避免模型尚未输出完整时提前修改文件。

use super::model::{Hunk, MAX_FILES, MAX_PATCH_BYTES, Operation};
use anyhow::{Result, bail, ensure};

/// 【文件补丁】【语法解析】解析新增、删除、移动和多个精确修改片段。
/// 参数: patch 为完整补丁文本；返回: 有序文件操作或带行号的格式错误。
pub(super) fn parse(patch: &str) -> Result<Vec<Operation>> {
    ensure!(
        patch.len() <= MAX_PATCH_BYTES,
        "patch exceeds the 1 MiB limit"
    );
    ensure!(
        !patch.contains('\0'),
        "patch must be UTF-8 text without NUL bytes"
    );
    let lines = patch
        .trim_end_matches(['\n', '\r'])
        .lines()
        .collect::<Vec<_>>();
    ensure!(
        lines.first() == Some(&"*** Begin Patch") && lines.last() == Some(&"*** End Patch"),
        "patch must start with *** Begin Patch and end with *** End Patch; do not use code fences"
    );
    let mut operations = Vec::new();
    let mut index = 1;
    while index + 1 < lines.len() {
        ensure!(
            operations.len() < MAX_FILES,
            "patch exceeds the 64-file limit"
        );
        let header = lines[index];
        index += 1;
        let operation = if let Some(path) = header.strip_prefix("*** Add File: ") {
            let path = path_text(path)?;
            let mut added = Vec::new();
            let mut no_newline = false;
            while index + 1 < lines.len() && !lines[index].starts_with("*** ") {
                if lines[index] == "\\ No newline at end of file" {
                    ensure!(
                        !added.is_empty() && !no_newline,
                        "invalid newline marker at line {}",
                        index + 1
                    );
                    no_newline = true;
                } else {
                    ensure!(
                        !no_newline,
                        "content after end-of-file marker at line {}",
                        index + 1
                    );
                    let value = lines[index].strip_prefix('+').ok_or_else(|| {
                        anyhow::anyhow!("Add File requires + lines at line {}", index + 1)
                    })?;
                    added.push(value);
                }
                index += 1;
            }
            let mut content = added.join("\n");
            if !added.is_empty() && !no_newline {
                content.push('\n');
            }
            Operation::Add { path, content }
        } else if let Some(path) = header.strip_prefix("*** Delete File: ") {
            Operation::Delete {
                path: path_text(path)?,
            }
        } else if let Some(path) = header.strip_prefix("*** Update File: ") {
            let path = path_text(path)?;
            let destination = lines
                .get(index)
                .and_then(|line| line.strip_prefix("*** Move to: "))
                .map(path_text)
                .transpose()?;
            if destination.is_some() {
                index += 1;
            }
            let mut hunks: Vec<Hunk> = Vec::new();
            while index + 1 < lines.len()
                && (lines[index] == "@@" || lines[index].starts_with("@@ "))
            {
                ensure!(
                    hunks.last().is_none_or(|hunk| !hunk.at_eof
                        && !hunk.old_no_newline
                        && !hunk.new_no_newline),
                    "end-of-file hunk must be the last hunk in a file"
                );
                hunks.push(parse_hunk(&lines, &mut index)?);
            }
            ensure!(
                destination.is_some() || !hunks.is_empty(),
                "Update File requires at least one @@ hunk: {path}"
            );
            Operation::Update {
                path,
                destination,
                hunks,
            }
        } else {
            bail!("unexpected patch header at line {index}: {header}");
        };
        operations.push(operation);
    }
    ensure!(!operations.is_empty(), "patch contains no file operations");
    Ok(operations)
}

/// 【文件补丁】【片段解析】记录新旧上下文和明确的文件末尾约束。
/// 参数: lines 为协议行，index 为可更新游标；返回: 一个完整修改片段。
fn parse_hunk(lines: &[&str], index: &mut usize) -> Result<Hunk> {
    let mut hunk = Hunk {
        anchor: lines[*index].strip_prefix("@@ ").map(str::to_string),
        ..Default::default()
    };
    *index += 1;
    let mut previous = None;
    let mut changed = false;
    while *index + 1 < lines.len() {
        let line = lines[*index];
        if line == "*** End of File" {
            hunk.at_eof = true;
            *index += 1;
            break;
        }
        if line.starts_with("*** ") || line == "@@" || line.starts_with("@@ ") {
            break;
        }
        if line == "\\ No newline at end of file" {
            match previous.take() {
                Some('-') => hunk.old_no_newline = true,
                Some('+') => hunk.new_no_newline = true,
                Some(' ') => {
                    hunk.old_no_newline = true;
                    hunk.new_no_newline = true;
                }
                _ => bail!(
                    "newline marker must follow a content line at line {}",
                    *index + 1
                ),
            }
        } else {
            let (kind, text) = line.split_at_checked(1).ok_or_else(|| {
                anyhow::anyhow!(
                    "blank context lines need a space prefix at line {}",
                    *index + 1
                )
            })?;
            ensure!(
                matches!(kind, " " | "+" | "-"),
                "invalid hunk prefix at line {}",
                *index + 1
            );
            if kind != "+" {
                ensure!(
                    !hunk.old_no_newline,
                    "old content follows newline marker at line {}",
                    *index + 1
                );
                hunk.old.push(text.to_string());
            }
            if kind != "-" {
                ensure!(
                    !hunk.new_no_newline,
                    "new content follows newline marker at line {}",
                    *index + 1
                );
                hunk.new.push(text.to_string());
            }
            previous = kind.chars().next();
            changed |= kind != " ";
        }
        *index += 1;
    }
    ensure!(
        changed,
        "hunk has no additions or deletions near line {}",
        *index
    );
    Ok(hunk)
}

/// 【文件补丁】【路径文本】拒绝隐藏控制字符和首尾空白，保留路径内部空格。
/// 参数: path 为原始路径；返回: 无歧义路径文本。
fn path_text(path: &str) -> Result<String> {
    ensure!(
        !path.is_empty() && path.trim() == path && !path.chars().any(char::is_control),
        "invalid patch path: {path:?}"
    );
    Ok(path.to_string())
}

use super::model::{ParsedFileChange, ParsedPatch, PatchHunk, PatchLine};
use anyhow::{bail, Result};
use std::path::PathBuf;

/// 解析 Codex 风格 patch 文本。
///
/// 参数:
/// - `patch`: `*** Begin Patch` 到 `*** End Patch` 的 patch 文本
///
/// 返回:
/// - 解析后的文件变更列表
pub(crate) fn parse_patch(patch: &str) -> Result<ParsedPatch> {
    let normalized = patch.replace("\r\n", "\n").replace('\r', "\n");
    let lines = normalized.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        bail!("patch is empty")
    }
    let mut index = skip_blank_lines(&lines, 0);
    if lines.get(index) != Some(&"*** Begin Patch") {
        bail!("patch must start with *** Begin Patch")
    }
    index += 1;

    let mut changes = Vec::new();
    loop {
        index = skip_blank_lines(&lines, index);
        let Some(line) = lines.get(index) else {
            bail!("patch is missing *** End Patch")
        };
        if *line == "*** End Patch" {
            break;
        }
        if let Some(path) = line.strip_prefix("*** Add File: ") {
            let (change, next_index) = parse_add_file(&lines, index + 1, path)?;
            changes.push(change);
            index = next_index;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Delete File: ") {
            changes.push(ParsedFileChange::Delete {
                path: clean_path(path)?,
            });
            index += 1;
            continue;
        }
        if let Some(path) = line.strip_prefix("*** Update File: ") {
            let (change, next_index) = parse_update_file(&lines, index + 1, path)?;
            changes.push(change);
            index = next_index;
            continue;
        }
        bail!("unsupported patch line: {line}")
    }

    Ok(ParsedPatch { changes })
}

/// 解析新增文件段。
///
/// 参数:
/// - `lines`: patch 行
/// - `index`: 内容起始行下标
/// - `path`: 文件路径文本
///
/// 返回:
/// - 新增文件变更和下一段下标
fn parse_add_file(
    lines: &[&str],
    mut index: usize,
    path: &str,
) -> Result<(ParsedFileChange, usize)> {
    let mut content = Vec::new();
    while let Some(line) = lines.get(index) {
        if is_section_header(line) {
            break;
        }
        let Some(text) = line.strip_prefix('+') else {
            bail!("Add File lines must start with +")
        };
        content.push(text.to_string());
        index += 1;
    }
    Ok((
        ParsedFileChange::Add {
            path: clean_path(path)?,
            content: join_patch_lines(&content),
        },
        index,
    ))
}

/// 解析更新文件段。
///
/// 参数:
/// - `lines`: patch 行
/// - `index`: 更新段起始行下标
/// - `path`: 文件路径文本
///
/// 返回:
/// - 更新文件变更和下一段下标
fn parse_update_file(
    lines: &[&str],
    mut index: usize,
    path: &str,
) -> Result<(ParsedFileChange, usize)> {
    let mut move_path = None;
    let mut hunks = Vec::new();
    while let Some(line) = lines.get(index) {
        if is_section_header(line) {
            break;
        }
        if let Some(path) = line.strip_prefix("*** Move to: ") {
            move_path = Some(clean_path(path)?);
            index += 1;
            continue;
        }
        if line.starts_with("@@") {
            let (hunk, next_index) = parse_hunk(lines, index + 1)?;
            hunks.push(hunk);
            index = next_index;
            continue;
        }
        bail!("Update File section expected @@ hunk or *** Move to")
    }
    if hunks.is_empty() && move_path.is_none() {
        bail!("Update File section must contain at least one hunk or move target")
    }
    Ok((
        ParsedFileChange::Update {
            path: clean_path(path)?,
            move_path,
            hunks,
        },
        index,
    ))
}

/// 解析单个 hunk。
///
/// 参数:
/// - `lines`: patch 行
/// - `index`: hunk 内容起始下标
///
/// 返回:
/// - hunk 和下一段下标
fn parse_hunk(lines: &[&str], mut index: usize) -> Result<(PatchHunk, usize)> {
    let mut hunk_lines = Vec::new();
    while let Some(line) = lines.get(index) {
        if is_section_header(line) || line.starts_with("@@") {
            break;
        }
        if line.starts_with("\\ No newline at end of file") {
            index += 1;
            continue;
        }
        let Some((prefix, text)) = split_patch_line(line) else {
            bail!("hunk lines must start with space, +, or -")
        };
        let patch_line = match prefix {
            ' ' => PatchLine::Context(text.to_string()),
            '+' => PatchLine::Add(text.to_string()),
            '-' => PatchLine::Delete(text.to_string()),
            _ => unreachable!(),
        };
        hunk_lines.push(patch_line);
        index += 1;
    }
    if hunk_lines.is_empty() {
        bail!("empty patch hunk")
    }
    Ok((PatchHunk { lines: hunk_lines }, index))
}

/// 切分 hunk 行前缀。
///
/// 参数:
/// - `line`: hunk 原始行
///
/// 返回:
/// - 前缀和正文
fn split_patch_line(line: &str) -> Option<(char, &str)> {
    let mut chars = line.chars();
    let prefix = chars.next()?;
    if matches!(prefix, ' ' | '+' | '-') {
        Some((prefix, chars.as_str()))
    } else {
        None
    }
}

/// 判断是否为 patch 段头。
///
/// 参数:
/// - `line`: patch 行
///
/// 返回:
/// - 是否为段头
fn is_section_header(line: &str) -> bool {
    line.starts_with("*** Add File: ")
        || line.starts_with("*** Delete File: ")
        || line.starts_with("*** Update File: ")
        || line == "*** End Patch"
}

/// 跳过空行。
///
/// 参数:
/// - `lines`: patch 行
/// - `index`: 当前下标
///
/// 返回:
/// - 第一个非空行下标
fn skip_blank_lines(lines: &[&str], mut index: usize) -> usize {
    while lines
        .get(index)
        .map(|line| line.trim().is_empty())
        .unwrap_or(false)
    {
        index += 1;
    }
    index
}

/// 清理 patch 路径。
///
/// 参数:
/// - `path`: 原始路径文本
///
/// 返回:
/// - 非空路径
fn clean_path(path: &str) -> Result<PathBuf> {
    let path = path.trim();
    if path.is_empty() {
        bail!("patch path is empty")
    }
    Ok(PathBuf::from(path))
}

/// 拼接 patch 内容行。
///
/// 参数:
/// - `lines`: 内容行
///
/// 返回:
/// - 带结尾换行的内容
fn join_patch_lines(lines: &[String]) -> String {
    if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_add_update_and_delete_sections() {
        let parsed = parse_patch(
            "*** Begin Patch\n*** Add File: a.txt\n+one\n*** Update File: b.txt\n@@\n-old\n+new\n*** Delete File: c.txt\n*** End Patch",
        )
        .unwrap();

        assert_eq!(parsed.changes.len(), 3);
    }
}

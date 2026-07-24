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
    let normalized = normalize_codex_patch(patch)?;
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

/// 归一化模型常见的 Codex patch 变体。
///
/// 兼容处理:
/// 1. 去掉 BOM / 首尾空白 / CR
/// 2. 去掉整段 Markdown 代码围栏
/// 3. 接受 `*** Begin Patch ***` / `*** End Patch ***` 等变体
/// 4. 从夹杂说明文字中截取 Begin..End 信封
/// 5. 仅有 section 头时自动补 Begin/End
///
/// 参数:
/// - `patch`: 原始 patch 文本
///
/// 返回:
/// - 标准 `*** Begin Patch` ... `*** End Patch` 文本
pub(crate) fn normalize_codex_patch(patch: &str) -> Result<String> {
    let mut text = patch.trim().trim_start_matches('\u{feff}').to_string();
    if text.is_empty() {
        bail!("patch is empty");
    }
    text = text.replace("\r\n", "\n").replace('\r', "\n");
    text = strip_outer_code_fence(&text);

    let lines = text.lines().collect::<Vec<_>>();
    let begin_idx = lines.iter().position(|line| is_begin_patch_line(line));
    let end_idx = lines.iter().rposition(|line| is_end_patch_line(line));

    let body = match (begin_idx, end_idx) {
        (Some(begin), Some(end)) if begin < end => {
            lines[begin + 1..end].join("\n")
        }
        (Some(begin), None) => {
            // 1. 有 Begin 无 End：若后续已是合法 section，补 End
            let rest = lines[begin + 1..].join("\n");
            if looks_like_patch_body(&rest) {
                rest
            } else {
                bail!(
                    "patch is missing *** End Patch; expected a complete Codex patch from *** Begin Patch through *** End Patch"
                );
            }
        }
        (None, Some(_)) => {
            bail!("patch has *** End Patch without *** Begin Patch");
        }
        (None, None) => {
            // 2. 完全无信封：整段看起来像 section 时自动包裹
            if looks_like_patch_body(&text) {
                text
            } else {
                bail!(
                    "patch must start with *** Begin Patch (also accepted: surrounding code fences, *** Begin Patch ***, or bare *** Update/Add/Delete File sections)"
                );
            }
        }
        _ => bail!("invalid patch envelope"),
    };

    let body = body.trim_matches('\n');
    if body.trim().is_empty() {
        bail!("patch body is empty");
    }
    Ok(format!("*** Begin Patch\n{body}\n*** End Patch"))
}

/// 去掉整段外层 Markdown 代码围栏。
///
/// 参数:
/// - `text`: 原始文本
///
/// 返回:
/// - 去掉围栏后的文本
fn strip_outer_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }
    let mut lines = trimmed.lines().collect::<Vec<_>>();
    if lines.len() < 2 {
        return trimmed.to_string();
    }
    // 1. 首行必须是 ``` 或 ```lang
    let first = lines[0].trim();
    if !first.starts_with("```") {
        return trimmed.to_string();
    }
    // 2. 末行必须是单独的 ```
    let last = lines[lines.len() - 1].trim();
    if last != "```" {
        return trimmed.to_string();
    }
    lines.remove(0);
    lines.pop();
    lines.join("\n")
}

/// 判断是否为 Begin Patch 行变体。
///
/// 参数:
/// - `line`: 原始行
///
/// 返回:
/// - 是否为 Begin 标记
fn is_begin_patch_line(line: &str) -> bool {
    let compact = line
        .trim()
        .trim_matches('*')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    compact == "begin patch"
}

/// 判断是否为 End Patch 行变体。
///
/// 参数:
/// - `line`: 原始行
///
/// 返回:
/// - 是否为 End 标记
fn is_end_patch_line(line: &str) -> bool {
    let compact = line
        .trim()
        .trim_matches('*')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    compact == "end patch"
}

/// 判断文本是否像合法的 Codex section 正文。
///
/// 参数:
/// - `text`: 候选正文
///
/// 返回:
/// - 是否像 patch body
fn looks_like_patch_body(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("*** Add File: ")
            || line.starts_with("*** Delete File: ")
            || line.starts_with("*** Update File: ")
    })
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

    #[test]
    fn normalizes_begin_end_variants_and_code_fence() {
        let raw = "```\n*** Begin Patch ***\n*** Add File: a.txt\n+one\n*** End Patch ***\n```";
        let parsed = parse_patch(raw).unwrap();
        assert_eq!(parsed.changes.len(), 1);
    }

    #[test]
    fn wraps_bare_update_section() {
        let raw = "*** Update File: b.txt\n@@\n-old\n+new";
        let parsed = parse_patch(raw).unwrap();
        assert_eq!(parsed.changes.len(), 1);
    }

    #[test]
    fn extracts_envelope_from_leading_noise() {
        let raw = "here is the patch:\n*** Begin Patch\n*** Add File: a.txt\n+one\n*** End Patch\nthanks";
        let parsed = parse_patch(raw).unwrap();
        assert_eq!(parsed.changes.len(), 1);
    }

    #[test]
    fn rejects_non_patch_text() {
        assert!(parse_patch("not a patch").is_err());
    }
}

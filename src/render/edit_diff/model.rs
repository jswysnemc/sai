use crate::i18n::text as t;
use crate::tools::edit_patch::{AppliedPatch, FileChange, LineChange, LineChangeKind};
use anyhow::{bail, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// 根据 edit_file 参数构造 diff 预览。
///
/// 参数:
/// - `arguments`: edit_file 工具参数 JSON
///
/// 返回:
/// - 可渲染的 patch 预览
pub(crate) fn preview_from_arguments(arguments: &str) -> Result<AppliedPatch> {
    let value = match serde_json::from_str::<Value>(arguments) {
        Ok(value) => value,
        Err(err) => {
            if let Some(patch) = string_field_from_partial(arguments, "patch") {
                return crate::tools::edit_patch::preview_patch(
                    &patch,
                    &crate::runtime_cwd::current_dir()?,
                );
            }
            return Err(err.into());
        }
    };
    if let Some(patch) = value.get("patch").and_then(Value::as_str) {
        return crate::tools::edit_patch::preview_patch(patch, &crate::runtime_cwd::current_dir()?);
    }
    if value.get("content").is_some() {
        return preview_full_file_edit(&value);
    }
    preview_line_edit(&value)
}

/// 从部分 JSON 参数中提取已闭合字符串字段。
///
/// 参数:
/// - `raw`: 原始 JSON 或 JSON 片段
/// - `key`: 字段名
///
/// 返回:
/// - 字符串字段内容，字段未闭合时返回空
fn string_field_from_partial(raw: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    let quote_index = after_colon.find('"')?;
    parse_json_string(&after_colon[quote_index..])
}

/// 解析 JSON 字符串片段。
///
/// 参数:
/// - `value`: 以双引号开头的 JSON 字符串片段
///
/// 返回:
/// - 解析后的字符串，未闭合时返回空
fn parse_json_string(value: &str) -> Option<String> {
    if !value.starts_with('"') {
        return None;
    }
    let mut output = String::new();
    let mut escaped = false;
    for ch in value.chars().skip(1) {
        if escaped {
            output.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(output);
        }
        output.push(ch);
    }
    None
}

/// 构造整文件编辑预览。
///
/// 参数:
/// - `value`: edit_file 工具参数
///
/// 返回:
/// - 文件变更预览
fn preview_full_file_edit(value: &Value) -> Result<AppliedPatch> {
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .map(expand_path)
        .ok_or_else(|| anyhow::anyhow!(t("path is required", "必须提供路径")))?;
    let content = value
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t("content is required", "必须提供内容")))?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if !path.exists() {
        return Ok(AppliedPatch {
            changes: vec![FileChange::Add { path, content }],
        });
    }
    let old_content = std::fs::read_to_string(&path)?;
    let lines = full_file_lines(&old_content, &content);
    Ok(AppliedPatch {
        changes: vec![FileChange::Update {
            path,
            move_path: None,
            new_content: content,
            lines,
        }],
    })
}

/// 构造行级编辑预览。
///
/// 参数:
/// - `value`: edit_file 工具参数
///
/// 返回:
/// - 文件变更预览
fn preview_line_edit(value: &Value) -> Result<AppliedPatch> {
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .map(expand_path)
        .ok_or_else(|| anyhow::anyhow!(t("path is required", "必须提供路径")))?;
    let start_line = value
        .get("start_line")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!(t("start_line is required", "必须提供 start_line")))?
        as usize;
    let end_line = value
        .get("end_line")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!(t("end_line is required", "必须提供 end_line")))?
        as usize;
    let replacement = value
        .get("replacement")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t("replacement is required", "必须提供 replacement")))?
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if start_line == 0 || end_line == 0 || start_line > end_line {
        bail!(t("invalid line range", "无效行范围"))
    }
    let old_content = std::fs::read_to_string(&path)?;
    let old_lines = old_content.lines().map(str::to_string).collect::<Vec<_>>();
    if start_line > old_lines.len() || end_line > old_lines.len() {
        bail!(t("line range out of range", "行范围超出文件范围"))
    }
    let replacement_lines = if replacement.is_empty() {
        Vec::new()
    } else {
        replacement.lines().map(str::to_string).collect::<Vec<_>>()
    };
    let mut new_lines = old_lines.clone();
    new_lines.splice(start_line - 1..end_line, replacement_lines.clone());
    let mut new_content = new_lines.join("\n");
    if old_content.ends_with('\n') && !new_content.is_empty() {
        new_content.push('\n');
    }
    let lines = line_edit_lines(&old_lines, start_line, end_line, &replacement_lines);
    Ok(AppliedPatch {
        changes: vec![FileChange::Update {
            path,
            move_path: None,
            new_content,
            lines,
        }],
    })
}

/// 构造整文件 diff 行。
///
/// 参数:
/// - `old_content`: 旧文件内容
/// - `new_content`: 新文件内容
///
/// 返回:
/// - diff 行
fn full_file_lines(old_content: &str, new_content: &str) -> Vec<LineChange> {
    let mut lines = Vec::new();
    for (index, line) in old_content.lines().enumerate() {
        lines.push(LineChange {
            kind: LineChangeKind::Delete,
            old_line: Some(index + 1),
            new_line: None,
            text: line.to_string(),
        });
    }
    for (index, line) in new_content.lines().enumerate() {
        lines.push(LineChange {
            kind: LineChangeKind::Add,
            old_line: None,
            new_line: Some(index + 1),
            text: line.to_string(),
        });
    }
    lines
}

/// 构造行级编辑 diff 行。
///
/// 参数:
/// - `old_lines`: 旧文件行
/// - `start_line`: 起始行号
/// - `end_line`: 结束行号
/// - `replacement_lines`: 替换行
///
/// 返回:
/// - 带上下文的 diff 行
fn line_edit_lines(
    old_lines: &[String],
    start_line: usize,
    end_line: usize,
    replacement_lines: &[String],
) -> Vec<LineChange> {
    let mut lines = Vec::new();
    if start_line > 1 {
        lines.push(LineChange {
            kind: LineChangeKind::Context,
            old_line: Some(start_line - 1),
            new_line: Some(start_line - 1),
            text: old_lines[start_line - 2].clone(),
        });
    }
    for line_number in start_line..=end_line {
        lines.push(LineChange {
            kind: LineChangeKind::Delete,
            old_line: Some(line_number),
            new_line: None,
            text: old_lines[line_number - 1].clone(),
        });
    }
    for (offset, line) in replacement_lines.iter().enumerate() {
        lines.push(LineChange {
            kind: LineChangeKind::Add,
            old_line: None,
            new_line: Some(start_line + offset),
            text: line.clone(),
        });
    }
    if end_line < old_lines.len() {
        lines.push(LineChange {
            kind: LineChangeKind::Context,
            old_line: Some(end_line + 1),
            new_line: Some(start_line + replacement_lines.len()),
            text: old_lines[end_line].clone(),
        });
    }
    lines
}

/// 展开路径。
///
/// 参数:
/// - `value`: 原始路径文本
///
/// 返回:
/// - 绝对路径或当前工作目录相对路径
fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

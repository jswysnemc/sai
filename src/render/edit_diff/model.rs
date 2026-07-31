use crate::render::terminal_text as t;
use crate::tools::edit_patch::{AppliedPatch, FileChange, LineChange, LineChangeKind};
use anyhow::{bail, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// 根据编辑类工具参数构造 diff 预览。
///
/// 参数:
/// - `arguments`: `edit_file` / `write_file` / `str_replace` 工具参数 JSON
///
/// 返回:
/// - 可渲染的变更预览
pub(crate) fn preview_from_arguments(arguments: &str) -> Result<AppliedPatch> {
    match serde_json::from_str::<Value>(arguments) {
        Ok(value) => preview_from_value(&value),
        Err(err) => {
            // 1. 流式 partial JSON：优先识别已闭合的 patch / write / replace 字段
            if let Some(preview) = preview_from_partial_arguments(arguments)? {
                return Ok(preview);
            }
            Err(err.into())
        }
    }
}

/// 从完整 JSON 参数构造 diff 预览。
///
/// 参数:
/// - `value`: 已解析的工具参数
///
/// 返回:
/// - 可渲染的变更预览
fn preview_from_value(value: &Value) -> Result<AppliedPatch> {
    if let Some(patch) = value.get("patch").and_then(Value::as_str) {
        let patch = patch.trim();
        if patch.is_empty() {
            bail!(t("patch is required", "必须提供 patch"));
        }
        return crate::tools::edit_patch::preview_patch(patch, &crate::runtime_cwd::current_dir()?);
    }
    if let (Some(path), Some(content)) = (
        value.get("path").and_then(Value::as_str),
        value.get("content").and_then(Value::as_str),
    ) {
        return preview_write_file(path, content);
    }
    if let (Some(path), Some(old_string), Some(new_string)) = (
        value.get("path").and_then(Value::as_str),
        value.get("old_string").and_then(Value::as_str),
        value.get("new_string").and_then(Value::as_str),
    ) {
        let replace_all = value
            .get("replace_all")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        return preview_str_replace(path, old_string, new_string, replace_all);
    }
    bail!(t(
        "edit arguments require patch, or path+content, or path+old_string+new_string",
        "编辑参数需要 patch，或 path+content，或 path+old_string+new_string"
    ))
}

/// 从流式 partial JSON 构造可渲染预览。
///
/// 参数:
/// - `raw`: 原始参数片段
///
/// 返回:
/// - 字段已闭合时返回预览；否则返回空
fn preview_from_partial_arguments(raw: &str) -> Result<Option<AppliedPatch>> {
    if let Some(patch) = string_field_from_partial(raw, "patch") {
        if !patch.trim().is_empty() {
            return Ok(Some(crate::tools::edit_patch::preview_patch(
                &patch,
                &crate::runtime_cwd::current_dir()?,
            )?));
        }
    }
    if let (Some(path), Some(content)) = (
        string_field_from_partial(raw, "path"),
        string_field_from_partial(raw, "content"),
    ) {
        if !path.trim().is_empty() {
            return Ok(Some(preview_write_file(&path, &content)?));
        }
    }
    if let (Some(path), Some(old_string), Some(new_string)) = (
        string_field_from_partial(raw, "path"),
        string_field_from_partial(raw, "old_string"),
        string_field_from_partial(raw, "new_string"),
    ) {
        if !path.trim().is_empty() {
            let replace_all = bool_field_from_partial(raw, "replace_all").unwrap_or(false);
            return Ok(Some(preview_str_replace(
                &path,
                &old_string,
                &new_string,
                replace_all,
            )?));
        }
    }
    Ok(None)
}

/// 预览 write_file 的新增或整文件覆盖。
///
/// 参数:
/// - `path_text`: 目标路径
/// - `content`: 新文件内容
///
/// 返回:
/// - 单文件变更预览
fn preview_write_file(path_text: &str, content: &str) -> Result<AppliedPatch> {
    let path = expand_path(path_text);
    if path.exists() && !path.is_file() {
        bail!("not a regular file: {}", path.display());
    }
    if !path.exists() {
        return Ok(AppliedPatch {
            changes: vec![FileChange::Add {
                path,
                content: content.to_string(),
            }],
        });
    }
    let old_content = std::fs::read_to_string(&path)?;
    Ok(AppliedPatch {
        changes: vec![FileChange::Update {
            path,
            move_path: None,
            new_content: content.to_string(),
            lines: build_line_diff(&old_content, content),
        }],
    })
}

/// 预览 str_replace 的局部替换。
///
/// 参数:
/// - `path_text`: 目标路径
/// - `old_string`: 待替换原文
/// - `new_string`: 替换后文本
/// - `replace_all`: 是否替换全部匹配
///
/// 返回:
/// - 单文件变更预览
fn preview_str_replace(
    path_text: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> Result<AppliedPatch> {
    if old_string.is_empty() {
        bail!("old_string must be non-empty");
    }
    if old_string == new_string {
        bail!("old_string and new_string are identical; no changes to make");
    }
    let path = expand_path(path_text);
    if !path.exists() {
        bail!("file not found: {}", path.display());
    }
    if !path.is_file() {
        bail!("not a regular file: {}", path.display());
    }
    let content = std::fs::read_to_string(&path)?;
    let matches = content.matches(old_string).count();
    if matches == 0 {
        bail!("String to replace not found in file");
    }
    if matches > 1 && !replace_all {
        bail!("Found {matches} matches of old_string, but replace_all is false");
    }
    let updated = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    Ok(AppliedPatch {
        changes: vec![FileChange::Update {
            path,
            move_path: None,
            new_content: updated.clone(),
            lines: build_line_diff(&content, &updated),
        }],
    })
}

/// 基于行的 LCS 构造可渲染 diff。
///
/// 参数:
/// - `old_content`: 旧文本
/// - `new_content`: 新文本
///
/// 返回:
/// - 带行号的增删上下文行
fn build_line_diff(old_content: &str, new_content: &str) -> Vec<LineChange> {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let (old_len, new_len) = (old_lines.len(), new_lines.len());
    // 1. 计算 LCS 长度表
    let mut dp = vec![vec![0usize; new_len + 1]; old_len + 1];
    for i in (0..old_len).rev() {
        for j in (0..new_len).rev() {
            dp[i][j] = if old_lines[i] == new_lines[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    // 2. 回溯生成带行号的 diff
    let mut lines = Vec::new();
    let mut i = 0usize;
    let mut j = 0usize;
    while i < old_len && j < new_len {
        if old_lines[i] == new_lines[j] {
            lines.push(LineChange {
                kind: LineChangeKind::Context,
                old_line: Some(i + 1),
                new_line: Some(j + 1),
                text: old_lines[i].to_string(),
            });
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            lines.push(LineChange {
                kind: LineChangeKind::Delete,
                old_line: Some(i + 1),
                new_line: None,
                text: old_lines[i].to_string(),
            });
            i += 1;
        } else {
            lines.push(LineChange {
                kind: LineChangeKind::Add,
                old_line: None,
                new_line: Some(j + 1),
                text: new_lines[j].to_string(),
            });
            j += 1;
        }
    }
    while i < old_len {
        lines.push(LineChange {
            kind: LineChangeKind::Delete,
            old_line: Some(i + 1),
            new_line: None,
            text: old_lines[i].to_string(),
        });
        i += 1;
    }
    while j < new_len {
        lines.push(LineChange {
            kind: LineChangeKind::Add,
            old_line: None,
            new_line: Some(j + 1),
            text: new_lines[j].to_string(),
        });
        j += 1;
    }
    // 3. 压缩为变更附近上下文，避免整文件重写刷屏
    compress_line_diff(&lines, 3)
}

/// 仅保留变更行附近上下文。
///
/// 参数:
/// - `lines`: 完整 diff 行
/// - `context`: 变更两侧保留的上下文行数
///
/// 返回:
/// - 压缩后的 diff 行
fn compress_line_diff(lines: &[LineChange], context: usize) -> Vec<LineChange> {
    if lines.is_empty() {
        return Vec::new();
    }
    let changed: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            matches!(line.kind, LineChangeKind::Add | LineChangeKind::Delete).then_some(index)
        })
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    let mut keep = vec![false; lines.len()];
    for &index in &changed {
        let start = index.saturating_sub(context);
        let end = (index + context + 1).min(lines.len());
        for slot in keep.iter_mut().take(end).skip(start) {
            *slot = true;
        }
    }
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| keep[index].then(|| line.clone()))
        .collect()
}

/// 展开 ~ 与相对路径。
///
/// 参数:
/// - `value`: 路径文本
///
/// 返回:
/// - 解析后的路径
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
    let quote_index = after_colon.find('\"')?;
    parse_json_string(&after_colon[quote_index..])
}

/// 从部分 JSON 参数中提取布尔字段。
///
/// 参数:
/// - `raw`: 原始 JSON 或 JSON 片段
/// - `key`: 字段名
///
/// 返回:
/// - 已出现的布尔值
fn bool_field_from_partial(raw: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{key}\"");
    let key_index = raw.find(&pattern)?;
    let after_key = &raw[key_index + pattern.len()..];
    let colon_index = after_key.find(':')?;
    let after_colon = after_key[colon_index + 1..].trim_start();
    if after_colon.starts_with("true") {
        Some(true)
    } else if after_colon.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// 解析 JSON 字符串片段。
///
/// 参数:
/// - `value`: 以双引号开头的 JSON 字符串片段
///
/// 返回:
/// - 解析后的字符串，未闭合时返回空
fn parse_json_string(value: &str) -> Option<String> {
    if !value.starts_with('\"') {
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
                '\"' => '\"',
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
        if ch == '\"' {
            return Some(output);
        }
        output.push(ch);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preview_requires_known_edit_fields() {
        let err = preview_from_arguments(r#"{"path":"a.rs"}"#).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("path+content")
                || message.contains("patch")
                || message.contains("old_string")
        );
    }

    #[test]
    fn preview_write_file_add_and_overwrite() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(&cwd).unwrap();
        let path = temp.path().join("notes.md");
        let add = preview_from_arguments(
            &json!({
                "path": path.display().to_string(),
                "content": "hello\nworld\n"
            })
            .to_string(),
        )
        .unwrap();
        assert!(matches!(add.changes[0], FileChange::Add { .. }));

        std::fs::write(&path, "hello\n").unwrap();
        let edit = preview_from_arguments(
            &json!({
                "path": path.display().to_string(),
                "content": "hello\nworld\n"
            })
            .to_string(),
        )
        .unwrap();
        match &edit.changes[0] {
            FileChange::Update { lines, .. } => {
                assert!(lines
                    .iter()
                    .any(|line| line.kind == LineChangeKind::Add && line.text == "world"));
            }
            other => panic!("expected update, got {other:?}"),
        }
    }

    #[test]
    fn preview_str_replace_shows_local_change() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(&cwd).unwrap();
        let path = temp.path().join("sample.rs");
        std::fs::write(&path, "fn main() {\n    old();\n}\n").unwrap();
        let preview = preview_from_arguments(
            &json!({
                "path": path.display().to_string(),
                "old_string": "    old();\n",
                "new_string": "    new();\n"
            })
            .to_string(),
        )
        .unwrap();
        match &preview.changes[0] {
            FileChange::Update { lines, .. } => {
                assert!(lines.iter().any(|line| {
                    line.kind == LineChangeKind::Delete && line.text.contains("old()")
                }));
                assert!(lines.iter().any(|line| {
                    line.kind == LineChangeKind::Add && line.text.contains("new()")
                }));
            }
            other => panic!("expected update, got {other:?}"),
        }
    }

    #[test]
    fn preview_partial_write_file_when_fields_closed() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(&cwd).unwrap();
        let path = temp.path().join("new.txt");
        let content = "alpha\nbeta\n";
        let content_json = serde_json::to_string(content).unwrap();
        let partial = format!(
            r#"{{"path":"{}","content":{},"extra":""#,
            path.display(),
            content_json
        );
        let preview = preview_from_arguments(&partial).unwrap();
        assert!(matches!(preview.changes[0], FileChange::Add { .. }));
    }
}

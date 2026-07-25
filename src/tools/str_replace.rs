use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 注册局部精确替换工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(
        ToolSpec::new(
            "str_replace",
            t(
                "Perform an exact string replacement in an existing text file. Prefer reading the file first. old_string must match exactly; if it matches multiple places, either expand old_string until unique or set replace_all=true. Prefer write_file for new files or full rewrites, and edit_file for multi-file Codex patches. Do not use shell redirection to edit source.",
                "对已有文本文件做精确字符串替换。修改前优先读取文件。old_string 必须精确匹配；若匹配多处，扩展上下文使其唯一，或设置 replace_all=true。新建或整文件重写用 write_file；多文件 Codex 补丁用 edit_file。不要用 shell 重定向改源码。",
            ),
            str_replace_parameters(),
            |args| async move { str_replace(args) },
        )
        .writes(),
    );
}

/// 返回 str_replace 参数 schema。
///
/// 返回:
/// - path/old_string/new_string 的 JSON Schema
fn str_replace_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": t(
                    "Path to an existing text file. Relative paths resolve from the workspace root.",
                    "已有文本文件路径。相对路径相对工作区根目录解析。"
                )
            },
            "old_string": {
                "type": "string",
                "description": t(
                    "Exact text to replace. Must be unique unless replace_all is true.",
                    "要替换的精确原文。除非 replace_all=true，否则必须在文件中唯一。"
                )
            },
            "new_string": {
                "type": "string",
                "description": t(
                    "Replacement text.",
                    "替换后的文本。"
                )
            },
            "replace_all": {
                "type": "boolean",
                "description": t(
                    "When true, replace every exact match of old_string. Defaults to false.",
                    "为 true 时替换所有匹配项，默认 false。"
                )
            }
        },
        "required": ["path", "old_string", "new_string"],
        "additionalProperties": false
    })
}

/// 执行局部精确替换。
///
/// 参数:
/// - `args`: path/old_string/new_string/replace_all
///
/// 返回:
/// - JSON 变更摘要
fn str_replace(args: Value) -> Result<String> {
    let path_text = required_string(&args, "path")?;
    let old_string = args
        .get("old_string")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("old_string is required"))?;
    let new_string = args
        .get("new_string")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("new_string is required"))?;
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if old_string == new_string {
        bail!("old_string and new_string are identical; no changes to make");
    }
    if old_string.is_empty() {
        bail!("old_string must be non-empty");
    }

    // 1. 读取目标文件
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
        bail!(
            "String to replace not found in file. Read the exact region again and copy old_string verbatim, including whitespace and indentation.\nString: {old_string}"
        );
    }
    if matches > 1 && !replace_all {
        bail!(
            "Found {matches} matches of old_string, but replace_all is false. Provide more surrounding context to make old_string unique, or set replace_all=true.\nString: {old_string}"
        );
    }

    // 2. 替换并写回
    let updated = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    write_text_file(&path, &updated)?;

    let replacements = if replace_all { matches } else { 1 };
    let old_lines = line_count(old_string) * replacements;
    let new_lines = line_count(new_string) * replacements;
    let added = new_lines;
    let removed = old_lines;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "mode": "replace",
        "replacements": replacements,
        "changed_files": [{
            "action": "Edited",
            "path": path.display().to_string(),
            "added": added,
            "removed": removed
        }]
    }))?)
}

/// 读取非空字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 字段名
///
/// 返回:
/// - 去掉首尾空白后的字符串
fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required"))
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

/// 原子写入 UTF-8 文本。
///
/// 参数:
/// - `path`: 目标路径
/// - `content`: 文件内容
///
/// 返回:
/// - 是否成功
fn write_text_file(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content.as_bytes())?;
    temp.persist(path)?;
    Ok(())
}

/// 统计文本行数。
///
/// 参数:
/// - `content`: 文本
///
/// 返回:
/// - 行数
fn line_count(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_replace_schema_requires_core_fields() {
        let schema = str_replace_parameters();
        assert_eq!(schema["required"], json!(["path", "old_string", "new_string"]));
    }

    #[test]
    fn str_replace_updates_unique_match() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        let result = str_replace(json!({
            "path": path.display().to_string(),
            "old_string": "one",
            "new_string": "ONE"
        }))
        .unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["changed_files"][0]["action"], "Edited");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "ONE\ntwo\n");
    }

    #[test]
    fn str_replace_rejects_ambiguous_match_without_replace_all() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "aa\naa\n").unwrap();
        let err = str_replace(json!({
            "path": path.display().to_string(),
            "old_string": "aa",
            "new_string": "bb"
        }))
        .unwrap_err()
        .to_string();
        assert!(err.contains("replace_all"));
    }
}

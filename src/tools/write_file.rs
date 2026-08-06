use super::file_diff::unified_diff;
use super::file_edit::atomic_write::write_text_file;
use super::fs_path::{expand_path, fs_error};
use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// 注册整文件写入工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(
        ToolSpec::new(
            "write_file",
            t(
                "Create a new text file or fully overwrite an existing text file. Use for new files or complete rewrites. Prefer str_replace for small local edits and edit_file for multi-file Codex patches. Do not use shell redirection, tee, or python heredocs to write source files.",
                "创建新文本文件，或整文件覆盖已有文本文件。用于新建或完整重写。局部小改优先 str_replace；多文件 Codex 补丁用 edit_file。不要用 shell 重定向、tee 或 python heredoc 写源码。",
            ),
            write_file_parameters(),
            |args| async move { write_file(args) },
        )
        .writes(),
    );
}

/// 返回 write_file 参数 schema。
///
/// 返回:
/// - path/content 的 JSON Schema
fn write_file_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": t(
                    "Path to the file to create or overwrite. Relative paths resolve from the workspace root.",
                    "要创建或覆盖的文件路径。相对路径相对工作区根目录解析。"
                )
            },
            "content": {
                "type": "string",
                "description": t(
                    "Raw full file content to write exactly as provided.",
                    "按原样写入的完整文件内容。"
                )
            },
            "mode": {
                "type": "string",
                "enum": ["overwrite", "append"],
                "description": t(
                    "Write mode. Defaults to overwrite. append adds content to the end exactly as provided and does not add a newline.",
                    "写入模式，默认 overwrite。append 按原样追加到末尾，不额外补换行。"
                )
            }
        },
        "required": ["path", "content"],
        "additionalProperties": false
    })
}

/// 整文件写入。
///
/// 参数:
/// - `args`: 必须包含 path 与 content
///
/// 返回:
/// - JSON 变更摘要
fn write_file(args: Value) -> Result<String> {
    let path_text = required_string(&args, "path")?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("content is required"))?;
    // 1. 解析路径并读取旧内容（若存在）
    let path = expand_path(path_text);
    let existed = path.exists();
    if existed && !path.is_file() {
        bail!("not a regular file: {}", path.display());
    }
    let old_content = if existed {
        std::fs::read_to_string(&path).map_err(|error| fs_error("read file", &path, &error))?
    } else {
        String::new()
    };
    let append = args
        .get("mode")
        .and_then(Value::as_str)
        .map(|mode| mode == "append")
        .unwrap_or(false);
    // 2. 追加模式接在原内容之后，覆盖模式整文件重写
    let final_content = if append {
        format!("{old_content}{content}")
    } else {
        content.to_string()
    };
    write_text_file(&path, &final_content)?;
    let (added, removed) = if existed {
        line_diff_counts(&old_content, &final_content)
    } else {
        (line_count(&final_content), 0)
    };
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "mode": if append { "append" } else { "write" },
        "changed_files": [{
            "action": if existed { "Edited" } else { "Added" },
            "path": path.display().to_string(),
            "added": added,
            "removed": removed
        }],
        "diff": unified_diff(path_text, &old_content, &final_content)
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

/// 粗略统计新旧文本的新增/删除行数。
///
/// 参数:
/// - `old_content`: 旧文本
/// - `new_content`: 新文本
///
/// 返回:
/// - `(added, removed)`
fn line_diff_counts(old_content: &str, new_content: &str) -> (usize, usize) {
    let old_lines = line_count(old_content);
    let new_lines = line_count(new_content);
    if new_lines >= old_lines {
        (new_lines - old_lines, 0)
    } else {
        (0, old_lines - new_lines)
    }
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
    fn write_file_schema_requires_path_and_content() {
        let schema = write_file_parameters();
        assert_eq!(schema["required"], json!(["path", "content"]));
        assert!(schema["properties"].get("patch").is_none());
    }

    #[test]
    fn write_file_creates_and_overwrites() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("nested").join("notes.md");
        let create = write_file(json!({
            "path": path.display().to_string(),
            "content": "# Title\n\nBody\n"
        }))
        .unwrap();
        let created: Value = serde_json::from_str(&create).unwrap();
        assert_eq!(created["changed_files"][0]["action"], "Added");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Title\n\nBody\n");

        let overwrite = write_file(json!({
            "path": path.display().to_string(),
            "content": "rewritten\n"
        }))
        .unwrap();
        let updated: Value = serde_json::from_str(&overwrite).unwrap();
        assert_eq!(updated["changed_files"][0]["action"], "Edited");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "rewritten\n");
    }

    /// append 模式按原样追加，不额外补换行。
    #[test]
    fn appends_content_without_adding_a_newline() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("log.txt");
        std::fs::write(&path, "first\n").unwrap();

        let output = write_file(json!({
            "path": path.display().to_string(),
            "content": "second\n",
            "mode": "append"
        }))
        .unwrap();

        assert!(output.contains("\"mode\": \"append\""));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "first\nsecond\n",
            "追加不得丢失原内容"
        );
    }

    /// 默认覆盖模式整文件重写。
    #[test]
    fn overwrites_by_default() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("log.txt");
        std::fs::write(&path, "old content\n").unwrap();

        write_file(json!({
            "path": path.display().to_string(),
            "content": "new\n"
        }))
        .unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
    }
}

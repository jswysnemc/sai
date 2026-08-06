use super::file_diff::unified_diff;
use super::file_edit::atomic_write::write_text_file;
use super::file_edit::line_endings::{materialize_model_text, to_model_text_view};
use super::fs_path::{expand_path, fs_error};
use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};

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
    let raw =
        std::fs::read_to_string(&path).map_err(|error| fs_error("read file", &path, &error))?;
    // 2. 纯 CRLF 文件按 LF 视图比对，模型给出的 old_string 才能匹配上
    let view = to_model_text_view(&raw);
    let content = view.text.as_str();
    let matches = content.matches(old_string).count();
    if matches == 0 {
        bail!(
            "old_string not found in {}, the file contents may be out of date. Please read the file again and copy old_string verbatim, including whitespace and indentation.",
            path.display()
        );
    }
    if matches > 1 && !replace_all {
        bail!(
            "old_string is not unique in {} (found {matches} occurrences). To replace every occurrence, set replace_all=true. To replace only one occurrence, include more surrounding context in old_string.",
            path.display()
        );
    }

    // 3. 替换后按原行尾风格写回
    let updated = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };
    write_text_file(
        &path,
        &materialize_model_text(&updated, view.line_ending_style),
    )?;

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
        }],
        "diff": unified_diff(path_text, content, &updated)
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
        assert_eq!(
            schema["required"],
            json!(["path", "old_string", "new_string"])
        );
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

    /// CRLF 文件按 LF 视图匹配，写回时还原 CRLF。
    #[test]
    fn replaces_inside_crlf_files_and_restores_line_endings() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("crlf.txt");
        std::fs::write(&path, "alpha\r\nbeta\r\n").unwrap();

        // 模型看到的是 LF 视图，因此 old_string 用 \n
        let output = str_replace(json!({
            "path": path.display().to_string(),
            "old_string": "alpha\nbeta",
            "new_string": "alpha\ngamma"
        }))
        .unwrap();

        assert!(output.contains("\"ok\": true"));
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written, "alpha\r\ngamma\r\n", "写回必须保持 CRLF");
    }

    /// LF 文件保持 LF，不被意外转换。
    #[test]
    fn keeps_lf_files_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("lf.txt");
        std::fs::write(&path, "alpha\nbeta\n").unwrap();

        str_replace(json!({
            "path": path.display().to_string(),
            "old_string": "beta",
            "new_string": "gamma"
        }))
        .unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\ngamma\n");
    }

    /// 未命中时给出可操作的提示，说明文件内容可能已过期。
    #[test]
    fn missing_old_string_reports_stale_content() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "alpha\n").unwrap();

        let error = str_replace(json!({
            "path": path.display().to_string(),
            "old_string": "missing",
            "new_string": "x"
        }))
        .unwrap_err()
        .to_string();

        assert!(error.contains("old_string not found"));
        assert!(error.contains("out of date"));
    }

    /// 多处命中且未设置 replace_all 时提示两种解法。
    #[test]
    fn ambiguous_old_string_suggests_both_options() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("dup.txt");
        std::fs::write(&path, "x\nx\n").unwrap();

        let error = str_replace(json!({
            "path": path.display().to_string(),
            "old_string": "x",
            "new_string": "y"
        }))
        .unwrap_err()
        .to_string();

        assert!(error.contains("not unique"));
        assert!(error.contains("replace_all=true"));
        assert!(error.contains("more surrounding context"));
    }
}

use super::edit_patch::{apply_patch, FileChange};
use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 注册编辑文件工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(
        ToolSpec::new(
            "edit_file",
            t(
                "Edit UTF-8 text files using exactly one mode. Patch mode uses only patch. Add File requires every content line, including blank lines, to begin with +. Update File requires an @@ hunk whose lines begin with space, +, or -. Full-file mode uses only path+content. Line-range mode uses only path+start_line+end_line+replacement after read_file confirms exact 1-based line numbers. Never mix fields from different modes.",
                "使用且仅使用一种模式编辑 UTF-8 文本文件。patch 模式只传 patch。Add File 的每一行内容（包括空行）都必须以 + 开头。Update File 必须包含 @@ hunk，hunk 每行以空格、+ 或 - 开头。整文件模式只传 path+content。行范围模式在 read_file 确认精确的 1 起始行号后，只传 path+start_line+end_line+replacement。禁止混用不同模式的字段。",
            ),
            edit_file_parameters(),
            |args| async move { edit_file(args) },
        )
        .writes(),
    );
    super::edit_file_tools::register(registry);
}

/**
 * 返回互斥的文件编辑参数模式。
 *
 * 返回:
 * - patch、整文件或行范围三选一的 JSON Schema
 */
fn edit_file_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "patch": {"type":"string","description": t("Preferred mode; pass no other fields. Add File example: *** Begin Patch\\n*** Add File: notes.md\\n+# Title\\n+\\n+Body\\n*** End Patch. Update File example: *** Begin Patch\\n*** Update File: src/main.rs\\n@@\\n-old\\n+new\\n*** End Patch.", "推荐模式，不要同时传其他字段。Add File 示例：*** Begin Patch\\n*** Add File: notes.md\\n+# Title\\n+\\n+Body\\n*** End Patch。Update File 示例：*** Begin Patch\\n*** Update File: src/main.rs\\n@@\\n-old\\n+new\\n*** End Patch。")},
            "path": {"type":"string","description": t("File path used only by full-file or line-range mode.", "仅供整文件模式或行范围模式使用的文件路径。")},
            "content": {"type":"string","description": t("Complete UTF-8 file content. Use only with path.", "完整 UTF-8 文件内容，只与 path 一起使用。")},
            "start_line": {"type":"integer","minimum":1,"description": t("First 1-based line to replace. Line-range mode only.", "要替换的第一行，1 起始。仅用于行范围模式。")},
            "end_line": {"type":"integer","minimum":1,"description": t("Last 1-based line to replace, inclusive. Line-range mode only.", "要替换的最后一行，1 起始且包含该行。仅用于行范围模式。")},
            "replacement": {"type":"string","description": t("Replacement text for line-range mode. Empty text deletes the selected range.", "行范围模式的替换文本。空文本删除所选行范围。")}
        },
        "additionalProperties": false
    })
}

/// 创建、覆盖或编辑 UTF-8 文本文件。
///
/// 参数:
/// - `args`: 工具参数，优先包含 patch；旧模式包含 content 或行号替换参数
///
/// 返回:
/// - JSON 格式编辑结果
fn edit_file(mut args: Value) -> Result<String> {
    normalize_edit_file_args(&mut args)?;
    validate_edit_file_mode(&args)?;
    if let Some(patch) = args.get("patch").and_then(Value::as_str) {
        return execute_patch(patch);
    }
    if args.get("content").is_some() {
        return write_file(args);
    }
    edit_file_lines(args)
}

/// 归一化兼容入口中的宽松模型参数。
///
/// 参数:
/// - `args`: 待归一化的编辑参数对象
///
/// 返回:
/// - 参数对象合法时返回成功
fn normalize_edit_file_args(args: &mut Value) -> Result<()> {
    let Some(object) = args.as_object_mut() else {
        bail!("edit_file arguments must be an object");
    };
    // 1. 部分模型会补齐未使用字段并传 null，这些字段不应参与模式判定
    object.retain(|_, value| !value.is_null());
    // 2. 兼容 JSON 中的数字字符串，避免行范围模式因类型漂移重试
    for key in ["start_line", "end_line"] {
        let parsed = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::parse::<u64>)
            .transpose()
            .map_err(|_| anyhow::anyhow!("{key} must be a positive integer"))?;
        if let Some(value) = parsed {
            object.insert(key.to_string(), json!(value));
        }
    }
    Ok(())
}

/// 应用 Codex 格式补丁。
///
/// 参数:
/// - `patch`: 完整补丁文本
///
/// 返回:
/// - JSON 格式文件变更摘要
pub(super) fn execute_patch(patch: &str) -> Result<String> {
    let cwd = crate::runtime_cwd::current_dir()?;
    let applied = apply_patch(patch, &cwd)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "mode": "patch",
        "changed_files": applied.changes.iter().map(file_change_summary).collect::<Vec<_>>()
    }))?)
}

/// 校验编辑文件参数只使用一种模式。
///
/// 参数:
/// - `args`: 编辑文件工具参数
///
/// 返回:
/// - 参数仅匹配 patch、整文件或行范围模式之一时成功
fn validate_edit_file_mode(args: &Value) -> Result<()> {
    let patch_mode = args.get("patch").is_some();
    let content_mode = args.get("content").is_some();
    let line_mode = ["start_line", "end_line", "replacement"]
        .iter()
        .any(|key| args.get(*key).is_some());
    let mode_count = [patch_mode, content_mode, line_mode]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if mode_count != 1 {
        bail!("provide exactly one edit mode: patch, path+content, or path+start_line+end_line+replacement")
    }
    if !patch_mode && args.get("path").is_none() {
        bail!("path is required for content and line-range edit modes")
    }
    Ok(())
}

/// 创建或覆盖 UTF-8 文本文件。
///
/// 参数:
/// - `args`: 工具参数，包含 path 和 content
///
/// 返回:
/// - JSON 格式写入结果
pub(super) fn write_file(args: Value) -> Result<String> {
    let path = path_arg(&args, "path")?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("content is required"))?;
    if path.exists() && !path.is_file() {
        bail!("not a regular file: {}", path.display())
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let old_bytes = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content.as_bytes())?;
    temp.persist(&path)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "mode": "content",
        "path": path.display().to_string(),
        "old_bytes": old_bytes,
        "new_bytes": content.len()
    }))?)
}

/// 按行号替换 UTF-8 文本文件。
///
/// 参数:
/// - `args`: 工具参数，包含 path、start_line、end_line 和 replacement
///
/// 返回:
/// - JSON 格式编辑结果
pub(super) fn edit_file_lines(args: Value) -> Result<String> {
    let path = path_arg(&args, "path")?;
    ensure_editable_file_path(&path)?;
    let start_line = args
        .get("start_line")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("start_line is required"))? as usize;
    let end_line = args
        .get("end_line")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow::anyhow!("end_line is required"))? as usize;
    let replacement = args
        .get("replacement")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("replacement is required"))?;
    if start_line == 0 || end_line == 0 {
        bail!("line numbers must be 1-based")
    }
    if start_line > end_line {
        bail!("start_line must be less than or equal to end_line")
    }
    let original = std::fs::read_to_string(&path)?;
    let had_trailing_newline = original.ends_with('\n');
    let mut lines = original.lines().map(str::to_string).collect::<Vec<_>>();
    let old_line_count = lines.len();
    if start_line > old_line_count || end_line > old_line_count {
        bail!("line range {start_line}-{end_line} out of range: {old_line_count} lines")
    }
    let replacement = replacement.replace("\r\n", "\n").replace('\r', "\n");
    let replacement_lines = if replacement.is_empty() {
        Vec::new()
    } else {
        replacement.lines().map(str::to_string).collect::<Vec<_>>()
    };
    lines.splice(start_line - 1..end_line, replacement_lines);
    let mut updated = lines.join("\n");
    if had_trailing_newline && !updated.is_empty() {
        updated.push('\n');
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), updated.as_bytes())?;
    temp.persist(&path)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "mode": "line",
        "path": path.display().to_string(),
        "old_line_count": old_line_count,
        "new_line_count": lines.len()
    }))?)
}

/// 生成文件变更摘要。
///
/// 参数:
/// - `change`: 文件变更
///
/// 返回:
/// - JSON 文件变更摘要
fn file_change_summary(change: &FileChange) -> Value {
    let (added, removed) = change.line_counts();
    let mut value = json!({
        "action": change.action_label(),
        "path": change.path().display().to_string(),
        "added": added,
        "removed": removed
    });
    if let FileChange::Update {
        move_path: Some(move_path),
        ..
    } = change
    {
        value["move_path"] = json!(move_path.display().to_string());
    }
    value
}

/// 确认路径是可编辑普通文件。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 校验是否成功
fn ensure_editable_file_path(path: &Path) -> Result<()> {
    let canonical = path.canonicalize()?;
    if !canonical.is_file() {
        bail!("not a regular file: {}", path.display())
    }
    Ok(())
}

/// 读取路径参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 展开后的路径
fn path_arg(args: &Value, key: &str) -> Result<PathBuf> {
    let value = required(args, key)?;
    Ok(expand_path(&value))
}

/// 展开工具路径。
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

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 非空参数值
fn required(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{}: {key}", t("required argument missing", "缺少必需参数"))
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_file_schema_uses_gateway_compatible_object_shape() {
        let schema = edit_file_parameters();
        assert!(schema.get("oneOf").is_none());
        assert!(schema.get("anyOf").is_none());
        assert!(schema.get("not").is_none());
        assert!(schema["properties"]["patch"]["description"]
            .as_str()
            .is_some_and(|value| value.contains("Add File") && value.contains("+# Title")));
    }

    #[test]
    fn edit_file_rejects_mixed_or_missing_modes() {
        assert!(validate_edit_file_mode(&json!({})).is_err());
        assert!(
            validate_edit_file_mode(&json!({"patch": "x", "path": "a", "content": "b"})).is_err()
        );
        assert!(validate_edit_file_mode(&json!({"path": "a", "start_line": 1})).is_ok());
    }

    #[test]
    fn write_file_creates_and_overwrites_text_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested").join("sample.txt");
        write_file(json!({"path": path.display().to_string(), "content": "one\n"})).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\n");
        let result =
            write_file(json!({"path": path.display().to_string(), "content": "two"})).unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["old_bytes"], 4);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "two");
    }

    #[test]
    fn edit_file_replaces_lines() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        edit_file(json!({
            "path": path.display().to_string(),
            "start_line": 2,
            "end_line": 2,
            "replacement": "TWO\nTWO-B"
        }))
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "one\nTWO\nTWO-B\nthree\n"
        );
    }

    #[test]
    fn edit_file_applies_patch() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        let patch = format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n-one\n+ONE\n two\n*** End Patch",
            path.display()
        );

        let result = edit_file(json!({ "patch": patch })).unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(data["changed_files"][0]["action"], "Edited");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "ONE\ntwo\n");
    }

    #[test]
    fn edit_file_ignores_null_fields_from_other_modes() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\n").unwrap();
        let patch = format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n-one\n+ONE\n*** End Patch",
            path.display()
        );

        edit_file(json!({
            "patch": patch,
            "path": null,
            "content": null,
            "start_line": null,
            "end_line": null,
            "replacement": null
        }))
        .unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "ONE\n");
    }

    #[test]
    fn edit_file_accepts_string_line_numbers() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();

        edit_file(json!({
            "path": path.display().to_string(),
            "start_line": "2",
            "end_line": "2",
            "replacement": "TWO"
        }))
        .unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "one\nTWO\n");
    }

    #[test]
    fn edit_file_allows_existing_files_outside_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        edit_file(json!({
            "path": path.display().to_string(),
            "start_line": 1,
            "end_line": 2,
            "replacement": "table"
        }))
        .unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "table\n");
    }
}

use super::edit_patch::{apply_patch, FileChange};
use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::Result;
use serde_json::{json, Value};

/// 注册仅支持 Codex patch 的文件编辑工具。
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
                "Apply one Codex-style multi-file patch. Preferred form starts with *** Begin Patch and ends with *** End Patch. Common model mistakes are auto-normalized: surrounding markdown fences, *** Begin Patch ***, leading chatter before the envelope, and bare *** Update/Add/Delete File sections. Supports Add File, Update File (with @@ hunks), Delete File, and Move. Every added content line in Add File (including blank lines) must start with +. Update hunk lines must start with space, +, or -. Prefer reading the file first so context lines match. Do not use shell redirection to edit source.",
                "向工作区应用一个 Codex 风格的多文件补丁。推荐以 *** Begin Patch 开头、*** End Patch 结束。以下常见模型误差会自动归一化：外层 Markdown 代码围栏、*** Begin Patch *** 变体、信封前说明文字、仅有 *** Update/Add/Delete File 段。支持 Add File、Update File（含 @@ hunk）、Delete File 和 Move。Add File 的每一行内容（包括空行）必须以 + 开头。Update hunk 每行以空格、+ 或 - 开头。修改前优先读取文件，确保上下文行匹配。不要用 shell 重定向改源码。",
            ),
            edit_file_parameters(),
            |args| async move { edit_file(args) },
        )
        .writes(),
    );
}

/// 返回仅 patch 模式的编辑参数 schema。
///
/// 返回:
/// - 只包含 patch 字段的 JSON Schema
fn edit_file_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "patch": {
                "type": "string",
                "description": t(
                    "Codex-style patch. Preferred: *** Begin Patch ... *** End Patch. Also accepts fenced blocks, *** Begin Patch ***, leading chatter, or bare *** Update/Add/Delete File sections. Example Add File:\n*** Begin Patch\n*** Add File: notes.md\n+# Title\n+\n+Body\n*** End Patch\nExample Update File:\n*** Begin Patch\n*** Update File: src/main.rs\n@@\n-old\n+new\n*** End Patch",
                    "Codex 格式补丁。推荐 *** Begin Patch ... *** End Patch。兼容代码围栏、*** Begin Patch *** 变体、信封前说明、仅 section 正文。Add File 示例：\n*** Begin Patch\n*** Add File: notes.md\n+# Title\n+\n+Body\n*** End Patch\nUpdate File 示例：\n*** Begin Patch\n*** Update File: src/main.rs\n@@\n-old\n+new\n*** End Patch"
                )
            }
        },
        "required": ["patch"],
        "additionalProperties": false
    })
}

/// 应用 Codex 格式补丁。
///
/// 参数:
/// - `args`: 工具参数，必须包含非空 patch
///
/// 返回:
/// - JSON 格式文件变更摘要
fn edit_file(args: Value) -> Result<String> {
    let patch = args
        .get("patch")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("patch is required"))?;
    // 1. 先归一化常见模型格式误差，再交给解析器
    let patch = super::edit_patch::normalize_codex_patch(patch)?;
    execute_patch(&patch)
}

/// 应用 Codex 格式补丁并生成结果。
///
/// 参数:
/// - `patch`: 完整补丁文本
///
/// 返回:
/// - JSON 格式文件变更摘要
fn execute_patch(patch: &str) -> Result<String> {
    let cwd = crate::runtime_cwd::current_dir()?;
    let applied = apply_patch(patch, &cwd)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "mode": "patch",
        "changed_files": applied.changes.iter().map(file_change_summary).collect::<Vec<_>>()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_file_schema_is_patch_only() {
        let schema = edit_file_parameters();
        assert_eq!(schema["required"], json!(["patch"]));
        assert_eq!(schema["additionalProperties"], json!(false));
        assert!(schema["properties"].get("path").is_none());
        assert!(schema["properties"].get("content").is_none());
        assert!(schema["properties"]["patch"]["description"]
            .as_str()
            .is_some_and(|value| {
                value.contains("Add File")
                    && value.contains("+# Title")
                    && (value.contains("*** Begin Patch") || value.contains("Begin Patch"))
            }));
    }

    #[test]
    fn registers_only_edit_file_tool() {
        let mut registry = ToolRegistry::new();
        register(&mut registry);
        let names = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["edit_file".to_string()]);
        assert_eq!(
            registry.definitions()[0].function.parameters["required"],
            json!(["patch"])
        );
    }

    #[test]
    fn edit_file_rejects_missing_or_invalid_patch() {
        assert!(edit_file(json!({})).is_err());
        assert!(edit_file(json!({"patch": ""})).is_err());
        assert!(edit_file(json!({"patch": "not a patch"})).is_err());
    }

    #[test]
    fn edit_file_accepts_common_model_patch_variants() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        let patch = format!(
            "```diff\n*** Begin Patch ***\n*** Update File: {}\n@@\n-one\n+ONE\n two\n*** End Patch ***\n```",
            path.display()
        );

        let result = edit_file(json!({ "patch": patch })).unwrap();
        let data: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(data["changed_files"][0]["action"], "Edited");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "ONE\ntwo\n");
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
    fn edit_file_adds_new_file_via_patch() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("nested").join("notes.md");
        let patch = format!(
            "*** Begin Patch\n*** Add File: {}\n+# Title\n+\n+Body\n*** End Patch",
            path.display()
        );

        edit_file(json!({ "patch": patch })).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "# Title\n\nBody\n");
    }
}

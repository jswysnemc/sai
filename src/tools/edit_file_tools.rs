use super::edit_file::{edit_file_lines, execute_patch, write_file};
use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// 注册单一模式的文件编辑工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(
        ToolSpec::new(
            "apply_patch",
            t(
                "Apply one Codex-style patch. Pass only the complete patch string from *** Begin Patch through *** End Patch.",
                "应用一个 Codex 格式补丁。只传从 *** Begin Patch 到 *** End Patch 的完整 patch 字符串。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "patch": {"type": "string", "description": t("Complete Codex-style patch.", "完整 Codex 格式补丁。")}
                },
                "required": ["patch"],
                "additionalProperties": false
            }),
            |args| async move { apply_patch_args(args) },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "write_file",
            t(
                "Create or overwrite one UTF-8 text file with complete content.",
                "使用完整内容创建或覆盖一个 UTF-8 文本文件。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": t("File path.", "文件路径。")},
                    "content": {"type": "string", "description": t("Complete file content; an empty string creates an empty file.", "完整文件内容；空字符串会创建空文件。")}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
            |args| async move { write_file(args) },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "replace_file_lines",
            t(
                "Replace an inclusive 1-based line range after read_file confirms the exact line numbers.",
                "在 read_file 确认精确行号后，替换一段包含首尾的 1 起始行范围。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": t("Existing UTF-8 file path.", "现有 UTF-8 文件路径。")},
                    "start_line": {"type": "integer", "minimum": 1, "description": t("First line to replace.", "替换起始行。")},
                    "end_line": {"type": "integer", "minimum": 1, "description": t("Last line to replace, inclusive.", "替换结束行，包含该行。")},
                    "replacement": {"type": "string", "description": t("Replacement text; an empty string deletes the range.", "替换文本；空字符串会删除该范围。")}
                },
                "required": ["path", "start_line", "end_line", "replacement"],
                "additionalProperties": false
            }),
            |args| async move { edit_file_lines(args) },
        )
        .writes(),
    );
}

/// 从工具参数中读取并执行补丁。
///
/// 参数:
/// - `args`: 包含 patch 的工具参数
///
/// 返回:
/// - JSON 格式文件变更摘要
fn apply_patch_args(args: Value) -> Result<String> {
    let patch = args
        .get("patch")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("patch is required"))?;
    if !patch.starts_with("*** Begin Patch") {
        bail!("patch must start with *** Begin Patch");
    }
    execute_patch(patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_single_mode_edit_tools_with_required_fields() {
        let mut registry = ToolRegistry::new();
        register(&mut registry);
        let definitions = registry.definitions();

        for (name, required) in [
            ("apply_patch", json!(["patch"])),
            ("write_file", json!(["path", "content"])),
            (
                "replace_file_lines",
                json!(["path", "start_line", "end_line", "replacement"]),
            ),
        ] {
            let definition = definitions
                .iter()
                .find(|definition| definition.function.name == name)
                .unwrap();
            assert_eq!(definition.function.parameters["required"], required);
            assert_eq!(
                definition.function.parameters["additionalProperties"],
                json!(false)
            );
        }
    }
}

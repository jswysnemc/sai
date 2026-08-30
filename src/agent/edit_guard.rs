use super::Agent;
use crate::llm::ToolCall;
use crate::tools::fs_path::expand_path;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::PathBuf;

/// 检查编辑工具目标的已知读取快照是否仍然有效。
///
/// 参数:
/// - `agent`: 当前 Agent
/// - `call`: 已通过参数 schema 校验的编辑调用
///
/// 返回:
/// - 未知读取来源或快照有效时成功；已知快照失效时返回阻断原因
pub(super) fn ensure_edit_target_was_read(agent: &Agent, call: &ToolCall) -> Result<()> {
    let args =
        super::first_json_object(&call.function.arguments).context("invalid tool arguments")?;
    let path_text = args
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;
    let path = expand_path(path_text);
    // 1. write_file 新建文件不需要先读不存在的目标
    if call.function.name == "write_file" && !path.exists() {
        return Ok(());
    }
    if !path.is_file() {
        bail!("edit target is not a regular file: {}", path.display());
    }
    // 2. 仅对本会话明确记录过的快照检查文件变化
    match agent.state().read_file_edit_block_reason(&path) {
        None => Ok(()),
        Some("changed") => bail!(
            "file changed after it was read: {}. Read the file again before editing.",
            path.display()
        ),
        // 模型可能通过 cat、PowerShell 或其他命令读取文件，无法仅凭工具记录判断
        Some(_) => Ok(()),
    }
}

/// 记录成功 read_file 调用中的全部文件目标。
///
/// 参数:
/// - `agent`: 当前 Agent
/// - `call`: read_file 调用
/// - `output`: 工具结果文本；错误结果不会登记
///
/// 返回:
/// - 登记结果
pub(super) fn record_successful_reads(agent: &Agent, call: &ToolCall, output: &str) -> Result<()> {
    if call.function.name != "read_file" || output.starts_with("tool error:") {
        return Ok(());
    }
    let args =
        super::first_json_object(&call.function.arguments).context("invalid tool arguments")?;
    for path in read_targets(&args) {
        if path.is_file() {
            agent.state().record_read_file(&path)?;
        }
    }
    Ok(())
}

/// 提取 read_file 的单文件或批量文件路径。
fn read_targets(args: &Value) -> Vec<PathBuf> {
    if let Some(files) = args.get("files").and_then(Value::as_array) {
        return files
            .iter()
            .filter_map(|item| item.get("path").and_then(Value::as_str))
            .map(expand_path)
            .collect();
    }
    args.get("path")
        .and_then(Value::as_str)
        .map(expand_path)
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_and_batch_targets() {
        let single = serde_json::json!({"path":"one.txt"});
        assert_eq!(read_targets(&single).len(), 1);
        let batch = serde_json::json!({"files":[{"path":"a"},{"path":"b"}]});
        assert_eq!(read_targets(&batch).len(), 2);
    }

    /// 门禁放行后这里会二次解析参数，必须用与门禁相同的规则，否则带残片的调用
    /// 会在登记读取记录时把错误冒到整轮之外。
    #[test]
    fn reparse_uses_the_same_tolerant_rule_as_the_gate() {
        let args = crate::agent::first_json_object(r#"{"path":"one.txt"} 残余片段"#).unwrap();

        assert_eq!(read_targets(&args), vec![expand_path("one.txt")]);
    }
}

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
    let targets = if call.function.name == "apply_patch" {
        crate::tools::apply_patch::existing_targets(
            args.get("patch")
                .and_then(Value::as_str)
                .context("patch is required")?,
        )?
    } else {
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
        vec![path]
    };
    for path in targets {
        if !path.is_file() {
            bail!("edit target is not a regular file: {}", path.display());
        }
        // 2. 仅对本会话明确记录过的快照检查文件变化
        match agent.state().read_file_edit_block_reason(&path) {
            None => {}
            Some("changed") => bail!(
                "file changed after it was read: {}. Read the file again before editing.",
                path.display()
            ),
            // 模型可能通过 cat、PowerShell 或其他命令读取文件，无法仅凭工具记录判断
            Some(_) => {}
        }
    }
    Ok(())
}

/// 记录成功 read_file 调用读取的文件。
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

/// 提取 read_file 读取的文件路径。
fn read_targets(args: &Value) -> Vec<PathBuf> {
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
    fn extracts_read_target() {
        let single = serde_json::json!({"path":"one.txt"});
        assert_eq!(read_targets(&single).len(), 1);
        assert!(read_targets(&serde_json::json!({})).is_empty());
    }

    /// 门禁放行后这里会二次解析参数，必须用与门禁相同的规则，否则带残片的调用
    /// 会在登记读取记录时把错误冒到整轮之外。
    #[test]
    fn reparse_uses_the_same_tolerant_rule_as_the_gate() {
        let args = crate::agent::first_json_object(r#"{"path":"one.txt"} 残余片段"#).unwrap();

        assert_eq!(read_targets(&args), vec![expand_path("one.txt")]);
    }

    /// 【文件补丁测试】【读取快照】批量修改检查每个已有源文件，任一旧快照失效就阻止整次调用。
    /// 参数: 无；返回: 无。
    #[tokio::test]
    async fn apply_patch_checks_every_previously_read_source() {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(root.path());
        paths.create_dirs().unwrap();
        let mut config = crate::config::AppConfig::default();
        config.skills.enabled = false;
        config.load_instruction_files = false;
        let state = crate::state::StateStore::new(&paths).unwrap();
        state.init_files().unwrap();
        let client = crate::llm::OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
        let agent = Agent::new(
            config,
            &paths,
            state.clone(),
            client,
            crate::tools::ToolRegistry::new(),
            super::super::AgentMode::Yolo,
        )
        .unwrap();
        for name in ["a", "b"] {
            let path = root.path().join(name);
            std::fs::write(&path, "old\n").unwrap();
            state.record_read_file(&path).unwrap();
        }
        let patch = format!("*** Begin Patch\n*** Update File: {}\n@@\n-old\n+new\n*** Delete File: {}\n*** Add File: {}\n+created\n*** End Patch", root.path().join("a").display(), root.path().join("b").display(), root.path().join("new").display());
        let call: ToolCall = serde_json::from_value(serde_json::json!({"id":"patch-read-guard", "type":"function", "function":{"name":"apply_patch", "arguments":serde_json::json!({"patch":patch}).to_string()}})).unwrap();
        ensure_edit_target_was_read(&agent, &call).unwrap();
        std::fs::write(root.path().join("b"), "externally changed\n").unwrap();
        let error = ensure_edit_target_was_read(&agent, &call)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("changed after it was read")
                && error.contains(&root.path().join("b").display().to_string()),
            "{error}"
        );
        assert!(!root.path().join("new").exists());
    }
}

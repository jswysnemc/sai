use super::*;
use crate::tools::subagent_persistence::SubagentExecutionContext;
use crate::tools::subagent_runner::checkpoint::SubagentCheckpoint;

/// 【子任务】【检查点读取】查询完整检查点；参数为任务标识，返回检查点或记录不存在错误。
pub(crate) fn load_checkpoint(id: &str) -> Result<Option<SubagentCheckpoint>> {
    let records = subagents().lock().expect("subagent state lock");
    let record = records
        .get(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    Ok(record.checkpoint.clone())
}

/// 【子任务】【检查点保存】在同一锁内保存对话和消息消费水位。
/// 参数：任务标识及完整检查点；返回磁盘保存结果，失败时不得继续执行工具。
pub(crate) fn save_checkpoint(id: &str, mut checkpoint: SubagentCheckpoint) -> Result<()> {
    let mut records = subagents().lock().expect("subagent state lock");
    let record = records
        .get_mut(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    checkpoint.consumed_messages = record.message_log.len();
    if let Some(result) = checkpoint.completed_result() {
        if record.snapshot.persistent
            && !record
                .checkpoint
                .as_ref()
                .is_some_and(|saved| saved.turn_complete)
        {
            record.snapshot.turns_completed += 1;
        }
        record.snapshot.result = Some(result.content);
        record.snapshot.stats = Some(checkpoint.stats.public());
    }
    record.checkpoint = Some(checkpoint);
    let owner = record.owner_key.clone();
    persistence::persist_owner_checked(&records, &owner)
}

/// 【子任务】【恢复环境】在启动前持久化执行目录；参数为任务标识及环境，返回保存结果。
pub(crate) fn set_execution_context(id: &str, execution: SubagentExecutionContext) -> Result<()> {
    let mut records = subagents().lock().expect("subagent state lock");
    let record = records
        .get_mut(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    record.execution = Some(execution);
    let owner = record.owner_key.clone();
    persistence::persist_owner_checked(&records, &owner)
}

/// 【子任务】【恢复认领】校验会话归属、状态及目录后原子认领中断任务，防止重复启动。
/// 参数：父会话作用域及子任务标识；返回快照、执行环境及新的取消通道。
pub(crate) fn prepare_resume(
    owner: &str,
    id: &str,
) -> Result<(
    SubagentSnapshot,
    SubagentExecutionContext,
    oneshot::Receiver<()>,
)> {
    ensure_owner_loaded(owner);
    let mut records = subagents().lock().expect("subagent state lock");
    let record = records
        .get_mut(id)
        .filter(|record| record.owner_key == owner)
        .ok_or_else(|| anyhow::anyhow!("subagent not found in current session: {id}"))?;
    anyhow::ensure!(
        record.snapshot.status == "interrupted",
        "only an interrupted subagent can be resumed"
    );
    anyhow::ensure!(
        record.checkpoint.is_some(),
        "this subagent has no recoverable conversation checkpoint"
    );
    let execution = record
        .execution
        .clone()
        .ok_or_else(|| anyhow::anyhow!("subagent execution context is missing"))?;
    anyhow::ensure!(
        execution.cwd.is_dir(),
        "subagent working directory no longer exists: {}",
        execution.cwd.display()
    );
    if let Some(worktree) = &execution.worktree {
        anyhow::ensure!(
            worktree.worktree_root.join(".git").exists(),
            "subagent worktree no longer exists"
        );
    }
    let (sender, receiver) = oneshot::channel();
    record.cancel = Some(sender);
    record.snapshot.status = "running".to_string();
    record.snapshot.error = None;
    record.snapshot.updated_at = unix_seconds();
    record.finish_notified = false;
    publish_record(record);
    let snapshot = record.snapshot.clone();
    persistence::persist_owner_checked(&records, owner)?;
    Ok((snapshot, execution, receiver))
}

/// 【子任务】【压缩留档】完整保存被压缩对话供回读，失败时保留原上下文。
/// 参数：任务标识和消息；返回独立留档路径。
pub(crate) fn archive_context(
    id: &str,
    messages: &[crate::llm::ChatMessage],
) -> Result<std::path::PathBuf> {
    let owner = {
        let records = subagents().lock().expect("subagent state lock");
        records
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?
            .owner_key
            .clone()
    };
    let dir = std::path::Path::new(&owner).join("subagent-context");
    std::fs::create_dir_all(&dir)?;
    let messages = messages
        .iter()
        .map(crate::tools::subagent_runner::checkpoint::SavedMessage::new)
        .collect::<Vec<_>>();
    let file = tempfile::Builder::new()
        .prefix("history-")
        .suffix(".json")
        .tempfile_in(dir)?;
    std::fs::write(file.path(), serde_json::to_vec_pretty(&messages)?)?;
    let (_, path) = file.keep()?;
    Ok(path)
}

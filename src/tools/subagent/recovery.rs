use super::*;

/// 【子任务】【中断恢复】恢复原工作目录内的检查点，并重新使用当前会话权限和工具设置。
/// 参数：恢复参数和父会话上下文；返回已启动任务快照。
pub(super) async fn resume_interrupted(
    args: Value,
    mut context: SubagentContext,
) -> Result<String> {
    let id = string_arg(&args, "subagent_id")?;
    runner_factory::refresh_model_settings(&mut context)?;
    let snapshot = subagent_state::subagent_snapshot_for_owner(&context.owner_key, &id)?;
    // 1. 【子任务】【权限校验】档案停用或模型配置失效时，不认领任务
    let _ = build_subagent_runner(
        &id,
        &snapshot.subagent_type,
        snapshot.max_steps,
        &context,
        ToolProgress::default(),
        snapshot.persistent,
    )?;
    let (snapshot, execution, cancel) = subagent_state::prepare_resume(&context.owner_key, &id)?;
    tokio::spawn(async move {
        crate::runtime_cwd::scope(
            execution.cwd,
            execute_subagent(id, String::new(), context, cancel, execution.worktree),
        )
        .await;
    });
    Ok(serde_json::to_string_pretty(
        &json!({"ok":true,"subagent":snapshot}),
    )?)
}

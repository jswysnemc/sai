use super::*;

/// 执行后台子智能体并写回状态。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `cancel_rx`: 取消信号接收器
///
/// 返回:
/// - 无
pub(super) async fn execute_subagent(
    subagent_id: String,
    prompt: String,
    context: SubagentContext,
    mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    worktree: Option<crate::tools::subagent_worktree::SubagentWorktree>,
) {
    let paths = context.paths.clone();
    let session_id = context.session_id.clone();
    let subagent = match subagent_state::subagent_snapshot(&subagent_id) {
        Ok(subagent) => subagent,
        Err(err) => {
            subagent_state::finish_subagent(
                &subagent_id,
                "failed",
                None,
                Some(err.to_string()),
                None,
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
            return;
        }
    };
    // 1. 起进度 channel，进度消息由 feed 解析写入时间线与快照，供前端实时渲染
    let (progress_tx, progress_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let mut progress_task = tokio::spawn(subagent_feed::consume_progress(
        subagent_id.clone(),
        progress_rx,
    ));
    let progress = ToolProgress::new(progress_tx);
    let result = tokio::select! {
        _ = &mut cancel_rx => Err(anyhow::anyhow!("cancelled")),
        result = run_subagent_session(&subagent, &prompt, context, progress) => result,
    };
    // 2. 运行结束会释放全部 sender，等待消费任务排空尾部流事件
    if tokio::time::timeout(Duration::from_secs(2), &mut progress_task)
        .await
        .is_err()
    {
        progress_task.abort();
    }
    // 3. 持久子智能体经 stop 结束时尊重其 apply 标志;一次性子智能体沿用成功即合并
    let apply_allowed = subagent_state::subagent_stop_requested(&subagent_id)
        .map(|stop| stop.apply)
        .unwrap_or(true);
    let merge_summary = finalize_worktree(
        &subagent_id,
        worktree.as_ref(),
        result.is_ok() && apply_allowed,
    );
    match result {
        Ok((content, stats)) => {
            let content = append_merge_summary(content, merge_summary.as_ref());
            subagent_state::finish_subagent(
                &subagent_id,
                "completed",
                Some(content),
                None,
                Some(stats),
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
        }
        Err(err) if err.to_string() == "cancelled" => {
            subagent_state::finish_subagent(
                &subagent_id,
                "cancelled",
                None,
                Some("cancelled".to_string()),
                None,
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
        }
        Err(err) => {
            subagent_state::finish_subagent(
                &subagent_id,
                "failed",
                None,
                Some(err.to_string()),
                None,
            );
            record_finished_runtime_subagent(&paths, &session_id, &subagent_id);
        }
    }
}

/// Apply (on success) and always clean up a delegated worktree.
fn finalize_worktree(
    subagent_id: &str,
    worktree: Option<&crate::tools::subagent_worktree::SubagentWorktree>,
    completed_ok: bool,
) -> Option<serde_json::Value> {
    let worktree = worktree?;
    let mut summary = serde_json::Map::new();
    summary.insert(
        "worktree_root".to_string(),
        json!(worktree.worktree_root.display().to_string()),
    );
    summary.insert("branch".to_string(), json!(worktree.branch_name.clone()));

    if completed_ok {
        match crate::tools::subagent_worktree::apply(worktree) {
            Ok(apply_result) => {
                summary.insert(
                    "apply".to_string(),
                    serde_json::to_value(&apply_result).unwrap_or(json!({})),
                );
            }
            Err(err) => {
                summary.insert("apply_error".to_string(), json!(err.to_string()));
            }
        }
    } else {
        summary.insert("apply_skipped".to_string(), json!("subagent_not_completed"));
    }

    let cleanup = crate::tools::subagent_worktree::cleanup(worktree);
    summary.insert(
        "cleanup".to_string(),
        serde_json::to_value(&cleanup).unwrap_or(json!({})),
    );
    let value = Value::Object(summary);
    subagent_state::set_subagent_worktree_merge(subagent_id, value.clone());
    Some(value)
}

fn append_merge_summary(content: String, merge: Option<&Value>) -> String {
    let Some(merge) = merge else {
        return content;
    };
    let applied = merge
        .pointer("/apply/applied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let changed = merge
        .pointer("/apply/changed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let method = merge
        .pointer("/apply/apply_method")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let apply_error = merge.get("apply_error").and_then(Value::as_str);
    let note = if let Some(error) = apply_error {
        format!(
            "

[worktree merge failed: {error}]"
        )
    } else if changed && applied {
        format!(
            "

[worktree changes auto-merged via {method}]"
        )
    } else if changed {
        "

[worktree had changes but auto-merge skipped or no-op]"
            .to_string()
    } else {
        String::new()
    };
    format!("{content}{note}")
}

/// 记录已结束子智能体的运行时状态。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `subagent_id`: 子智能体 ID
///
/// 返回:
/// - 无
fn record_finished_runtime_subagent(paths: &SaiPaths, session_id: &str, subagent_id: &str) {
    if let Ok(subagent) = subagent_state::subagent_snapshot(subagent_id) {
        let _ = subagent_runtime::record_subagent_finished(paths, session_id, &subagent);
    }
}

/// 按子智能体的持久标记分发到一次性或持久会话执行路径。
///
/// 参数:
/// - `subagent`: 子智能体启动快照
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 子代理输出内容和公开统计信息
async fn run_subagent_session(
    subagent: &subagent_state::SubagentSnapshot,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    if subagent.persistent {
        run_persistent_subagent(
            &subagent.id,
            &subagent.subagent_type,
            subagent.max_steps,
            prompt,
            context,
            tool_progress,
        )
        .await
    } else {
        run_subagent(
            &subagent.id,
            &subagent.subagent_type,
            subagent.max_steps,
            prompt,
            context,
            tool_progress,
        )
        .await
    }
}

/// 运行指定类型的子代理。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
/// - `prompt`: 子智能体提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 子代理输出内容和公开统计信息
async fn run_subagent(
    subagent_id: &str,
    subagent_type: &str,
    max_steps: usize,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    let runner = build_subagent_runner(
        subagent_id,
        subagent_type,
        max_steps,
        &context,
        tool_progress,
        false,
    )?;
    let result = tokio::time::timeout(
        Duration::from_secs(SUBAGENT_TIMEOUT_SECONDS),
        runner.run(prompt),
    )
    .await
    .map_err(|_| anyhow::anyhow!("subagent timed out after {SUBAGENT_TIMEOUT_SECONDS}s"))??;
    let (chat_result, stats) = result;
    if chat_result.content.trim().is_empty() {
        bail!("subagent returned an empty result");
    }
    Ok((chat_result.content, stats_json(&stats)))
}

/// 持久子智能体待命轮询间隔（毫秒）。
const IDLE_POLL_MILLIS: u64 = 300;

/// 运行持久子智能体的多任务段会话循环。
///
/// 生命周期:running（执行任务段）与 idle（待命等新消息）交替，
/// 直到收到 stop 请求（段间生效，不打断段内工具调用）或被取消。
/// 对话历史与渐进加载状态跨任务段共享，统计信息累计。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 每个任务段的最大工具调用次数
/// - `prompt`: 首个任务段的提示
/// - `context`: 子智能体上下文
/// - `tool_progress`: 写回快照的进度上报通道
///
/// 返回:
/// - 最后一个任务段的输出内容和累计统计信息
async fn run_persistent_subagent(
    subagent_id: &str,
    subagent_type: &str,
    max_steps: usize,
    prompt: &str,
    context: SubagentContext,
    tool_progress: ToolProgress,
) -> Result<(String, Value)> {
    let runner = build_subagent_runner(
        subagent_id,
        subagent_type,
        max_steps,
        &context,
        tool_progress,
        true,
    )?;
    let (mut messages, mut stats, mut tool_visibility) = runner.restore_session(prompt)?;
    if let Some(saved) =
        subagent_state::load_checkpoint(subagent_id)?.filter(|saved| saved.turn_complete)
    {
        let snapshot = subagent_state::subagent_snapshot(subagent_id)?;
        let content = snapshot.result.unwrap_or_default();
        subagent_state::park_subagent(
            subagent_id,
            Some(content.clone()),
            Some(stats_json(&saved.stats)),
        );
        loop {
            if subagent_state::subagent_stop_requested(subagent_id).is_some() {
                return Ok((content, stats_json(&saved.stats)));
            }
            if subagent_state::subagent_inbox_len(subagent_id) > 0 {
                subagent_state::resume_subagent(subagent_id);
                break;
            }
            tokio::time::sleep(Duration::from_millis(IDLE_POLL_MILLIS)).await;
        }
    }
    loop {
        // 1. 跑一个任务段;段级超时兜住单段挂死,超时按失败处理
        let result = tokio::time::timeout(
            Duration::from_secs(SUBAGENT_TIMEOUT_SECONDS),
            runner.run_turn(&mut messages, &mut stats, &mut tool_visibility),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!("subagent segment timed out after {SUBAGENT_TIMEOUT_SECONDS}s")
        })??;
        let content = if result.content.trim().is_empty() {
            t(
                "(no text output for this segment)",
                "（本任务段无正文输出）",
            )
            .to_string()
        } else {
            result.content
        };
        let stats_value = stats_json(&stats);
        // 2. 段间检查结束请求:stop 不打断段内执行,在段完成后生效
        if subagent_state::subagent_stop_requested(subagent_id).is_some() {
            return Ok((content, stats_value));
        }
        // 3. 转入待命;转入失败说明已被取消等,直接交回当前段结果
        if !subagent_state::park_subagent(
            subagent_id,
            Some(content.clone()),
            Some(stats_value.clone()),
        ) {
            return Ok((content, stats_value));
        }
        // 4. 待命循环:等待追加消息或结束请求;取消由外层 select 直接中止
        loop {
            if subagent_state::subagent_stop_requested(subagent_id).is_some() {
                return Ok((content, stats_value));
            }
            if subagent_state::subagent_inbox_len(subagent_id) > 0 {
                subagent_state::resume_subagent(subagent_id);
                break;
            }
            tokio::time::sleep(Duration::from_millis(IDLE_POLL_MILLIS)).await;
        }
    }
}

use super::*;

/// 创建后台子智能体记录。
///
/// 参数:
/// - `description`: 任务描述
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
///
/// 返回:
/// - 子智能体快照和取消接收器
#[cfg(test)]
pub(crate) fn create_subagent(
    description: String,
    subagent_type: String,
    max_steps: usize,
) -> (SubagentSnapshot, oneshot::Receiver<()>) {
    create_subagent_for_owner("default", description, subagent_type, max_steps)
}

/// 创建绑定到父会话的后台子智能体记录。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `description`: 任务描述
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
///
/// 返回:
/// - 子智能体快照和取消接收器
#[cfg(test)]
pub(crate) fn create_subagent_for_owner(
    owner_key: &str,
    description: String,
    subagent_type: String,
    max_steps: usize,
) -> (SubagentSnapshot, oneshot::Receiver<()>) {
    create_subagent_for_owner_goal(owner_key, None, description, subagent_type, max_steps)
}

/// 创建绑定到父会话和持续目标的后台子智能体记录。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `goal_id`: 当前持续目标标识
/// - `description`: 任务描述
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
///
/// 返回:
/// - 子智能体快照和取消接收器
pub(crate) fn create_subagent_for_owner_goal(
    owner_key: &str,
    goal_id: Option<String>,
    description: String,
    subagent_type: String,
    max_steps: usize,
) -> (SubagentSnapshot, oneshot::Receiver<()>) {
    ensure_owner_loaded(owner_key);
    let now = unix_seconds();
    let id = format!("subagent_{now}_{}", rand::random::<u16>());
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let snapshot = SubagentSnapshot {
        id: id.clone(),
        goal_id,
        description,
        subagent_type,
        status: "running".to_string(),
        max_steps,
        started_at: now,
        updated_at: now,
        step: 0,
        phase: None,
        last_tool: None,
        result: None,
        error: None,
        stats: None,
        worktree_root: None,
        worktree_branch: None,
        parent_workdir: None,
        worktree_merge: None,
    };
    let mut record = SubagentRecord {
        owner_key: owner_key.to_string(),
        snapshot: snapshot.clone(),
        cancel: Some(cancel_tx),
        finish_notified: false,
        timeline: SubagentTimeline::default(),
        event_journal: SubagentEventJournal::new(),
    };
    publish_record(&mut record);
    let mut subagents = subagents().lock().expect("subagent state lock");
    subagents.insert(id, record);
    persist_owner_locked(&subagents, owner_key);
    (snapshot, cancel_rx)
}

/// Attach worktree isolation metadata to a running subagent.
pub(crate) fn set_subagent_worktree(
    id: &str,
    worktree_root: Option<String>,
    worktree_branch: Option<String>,
    parent_workdir: Option<String>,
) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    record.snapshot.worktree_root = worktree_root;
    record.snapshot.worktree_branch = worktree_branch;
    record.snapshot.parent_workdir = parent_workdir;
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// Attach worktree merge result metadata to a finished or finishing subagent.
pub(crate) fn set_subagent_worktree_merge(id: &str, merge: Value) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    record.snapshot.worktree_merge = Some(merge);
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 完成后台子智能体记录。
///
/// 参数:
/// - `id`: 任务 ID
/// - `status`: 完成状态
/// - `result`: 子代理结果
/// - `error`: 错误信息
/// - `stats`: 统计信息
///
/// 返回:
/// - 无
pub(crate) fn finish_subagent(
    id: &str,
    status: &str,
    result: Option<String>,
    error: Option<String>,
    stats: Option<Value>,
) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    record.snapshot.status = status.to_string();
    record.snapshot.updated_at = unix_seconds();
    record.snapshot.result = result;
    record.snapshot.error = error;
    record.snapshot.stats = stats;
    record.cancel = None;
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 更新运行中子智能体的中间进度。
///
/// 参数:
/// - `id`: 任务 ID
/// - `update`: 本次进度更新(步数、阶段、最近工具)
///
/// 返回:
/// - 无
pub(crate) fn update_subagent_progress(id: &str, update: SubagentProgressUpdate) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return;
    };
    // 1. 只更新运行中的子智能体，避免覆盖已写入的终态
    if record.snapshot.status != "running" {
        return;
    }
    if let Some(step) = update.step {
        record.snapshot.step = step;
    }
    if let Some(phase) = update.phase {
        record.snapshot.phase = Some(phase);
    }
    if let Some(last_tool) = update.last_tool {
        record.snapshot.last_tool = Some(last_tool);
    }
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 记录子智能体的一次子工具调用开始,并同步快照进度。
///
/// 参数:
/// - `id`: 任务 ID
/// - `name`: 子工具名称
/// - `args`: 子工具参数 JSON 文本
///
/// 返回:
/// - 无
pub(crate) fn timeline_tool_started(id: &str, name: &str, args: &str) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = running_record(&mut subagents, id) else {
        return;
    };
    let step = record.timeline.push_tool(name, args);
    record.snapshot.step = step;
    record.snapshot.last_tool = Some(name.to_string());
    record.snapshot.phase = Some(if is_zh() {
        format!("工具 #{step}：{} 运行中", readable_tool_name(name))
    } else {
        format!("tool #{step}: {name} running")
    });
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 回填子智能体最近一次子工具调用的结果,并同步快照进度。
///
/// 参数:
/// - `id`: 任务 ID
/// - `name`: 子工具名称
/// - `ok`: 是否成功
/// - `output`: 子工具输出
///
/// 返回:
/// - 无
pub(crate) fn timeline_tool_finished(id: &str, name: &str, ok: bool, output: &str) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = running_record(&mut subagents, id) else {
        return;
    };
    let Some(step) = record.timeline.complete_tool(name, ok, output) else {
        return;
    };
    let state_text = if ok {
        "ok"
    } else if is_zh() {
        "失败"
    } else {
        "failed"
    };
    record.snapshot.phase = Some(if is_zh() {
        format!("工具 #{step}：{} {state_text}", readable_tool_name(name))
    } else {
        format!("tool #{step}: {name} {state_text}")
    });
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 追加子智能体的正文或推理片段到时间线。
///
/// 参数:
/// - `id`: 任务 ID
/// - `text`: 文本片段
/// - `reasoning`: 是否为推理片段
///
/// 返回:
/// - 无
pub(crate) fn timeline_streaming_text(id: &str, text: &str, reasoning: bool) {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = running_record(&mut subagents, id) else {
        return;
    };
    if reasoning {
        record.timeline.append_reasoning(text);
    } else {
        record.timeline.append_text(text);
    }
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

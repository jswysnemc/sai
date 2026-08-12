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
    create_subagent_for_owner_goal(owner_key, None, description, subagent_type, max_steps, false)
}

/// 创建绑定到父会话和持续目标的后台子智能体记录。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `goal_id`: 当前持续目标标识
/// - `description`: 任务描述
/// - `subagent_type`: 子代理类型
/// - `max_steps`: 最大工具调用次数
/// - `persistent`: 是否为持久子智能体（任务段完成后进入待命而非终态）
///
/// 返回:
/// - 子智能体快照和取消接收器
pub(crate) fn create_subagent_for_owner_goal(
    owner_key: &str,
    goal_id: Option<String>,
    description: String,
    subagent_type: String,
    max_steps: usize,
    persistent: bool,
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
        persistent,
        pending_messages: 0,
        turns_completed: 0,
    };
    let mut record = SubagentRecord {
        owner_key: owner_key.to_string(),
        snapshot: snapshot.clone(),
        cancel: Some(cancel_tx),
        finish_notified: false,
        timeline: SubagentTimeline::default(),
        event_journal: SubagentEventJournal::new(),
        inbox: Vec::new(),
        message_log: Vec::new(),
        stop_request: None,
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
    // 1. 已进入终态的记录不再覆盖,避免取消与自然完成竞争时状态被回写
    if record.snapshot.status != "running" && record.snapshot.status != "idle" {
        return;
    }
    // 2. idle 段的完成通知此前可能已投递并确认;进入终态是新事件,重新触发一次提醒
    if record.snapshot.status == "idle" {
        record.finish_notified = false;
    }
    record.snapshot.status = status.to_string();
    record.snapshot.updated_at = unix_seconds();
    record.snapshot.result = result;
    record.snapshot.error = error;
    record.snapshot.stats = stats;
    record.cancel = None;
    // 3. 终态后不再有注入时机,把残余队列消息归档进历史
    if !record.inbox.is_empty() {
        let leftover = std::mem::take(&mut record.inbox);
        record.message_log.extend(leftover);
    }
    record.snapshot.pending_messages = 0;
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
}

/// 把运行中的持久子智能体转入 idle 待命态。
///
/// 写入当前任务段的结果与统计，累计已完成段数，并重置完成通知标记，
/// 让主 Agent 在下一个消息间隙收到"任务段完成"的自动提醒。
///
/// 参数:
/// - `id`: 任务 ID
/// - `result`: 当前任务段的最终正文
/// - `stats`: 截至当前段的累计统计信息
///
/// 返回:
/// - 成功转入待命返回 true；记录不存在或已离开 running 态返回 false
pub(crate) fn park_subagent(id: &str, result: Option<String>, stats: Option<Value>) -> bool {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return false;
    };
    // 1. 只有运行中的记录才能待命,避免覆盖 cancel 等已写入的终态
    if record.snapshot.status != "running" {
        return false;
    }
    record.snapshot.status = "idle".to_string();
    record.snapshot.result = result;
    record.snapshot.stats = stats;
    record.snapshot.turns_completed += 1;
    record.snapshot.phase = Some(if is_zh() {
        "待命中，等待追加消息".to_string()
    } else {
        "idle, waiting for follow-up messages".to_string()
    });
    record.snapshot.updated_at = unix_seconds();
    // 2. 每个任务段完成都重新触发一次完成通知
    record.finish_notified = false;
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
    true
}

/// 把待命中的持久子智能体唤醒回 running 态。
///
/// worker 在收到新的追加消息后调用；消息本体由运行循环在步间注入。
///
/// 参数:
/// - `id`: 任务 ID
///
/// 返回:
/// - 成功唤醒返回 true；记录不存在或不处于 idle 态返回 false
pub(crate) fn resume_subagent(id: &str) -> bool {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return false;
    };
    if record.snapshot.status != "idle" {
        return false;
    }
    record.snapshot.status = "running".to_string();
    record.snapshot.phase = Some(if is_zh() {
        "处理追加消息中".to_string()
    } else {
        "processing follow-up messages".to_string()
    });
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
    true
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

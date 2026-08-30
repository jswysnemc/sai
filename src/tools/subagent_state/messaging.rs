use super::*;

/// 向子智能体的消息队列投递一条追加消息。
///
/// 只有存活（running 或 idle）的子智能体接受消息；消息会在其
/// 运行循环的下一个步间间隙注入对话，不会打断进行中的工具调用。
///
/// 参数:
/// - `id`: 子智能体 ID
/// - `from`: 消息来源，`parent`（主代理）或 `user`（用户留言）
/// - `text`: 消息正文
///
/// 返回:
/// - 入队后的子智能体快照；记录不存在或已进入终态时报错
pub(crate) fn queue_subagent_message(id: &str, from: &str, text: &str) -> Result<SubagentSnapshot> {
    queue_subagent_mesh_message_in(id, from, text, MeshMessageMeta::default())
}

/// 把一条带网格关联信息的消息投递给子智能体。
///
/// 网格投递只按子智能体 id 定位，不校验父会话归属：归属由网格工具在调用前
/// 按 `mesh.cross_session` 判定，这里不再重复。子智能体已进入终态时报错，
/// 调用方据此回退到磁盘信箱。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `id`: 子智能体 ID
/// - `from`: 消息来源标识
/// - `text`: 消息正文
/// - `meta`: 网格关联信息
///
/// 返回:
/// - 入队后的子智能体快照；记录不存在或已进入终态时报错
pub(crate) fn queue_subagent_mesh_message(
    owner_key: &str,
    id: &str,
    from: &str,
    text: &str,
    meta: MeshMessageMeta,
) -> Result<SubagentSnapshot> {
    ensure_owner_loaded(owner_key);
    queue_subagent_mesh_message_in(id, from, text, meta)
}

/// 向子智能体消息队列压入一条消息。
///
/// 参数:
/// - `id`: 子智能体 ID
/// - `from`: 消息来源标识
/// - `text`: 消息正文
/// - `meta`: 网格关联信息；非网格消息传默认值
///
/// 返回:
/// - 入队后的子智能体快照；记录不存在或已进入终态时报错
fn queue_subagent_mesh_message_in(
    id: &str,
    from: &str,
    text: &str,
    meta: MeshMessageMeta,
) -> Result<SubagentSnapshot> {
    let text = text.trim();
    if text.is_empty() {
        bail!("subagent message must not be empty");
    }
    let mut subagents = subagents().lock().expect("subagent state lock");
    let record = subagents
        .get_mut(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    if record.snapshot.status != "running" && record.snapshot.status != "idle" {
        bail!(
            "subagent is not accepting messages (status={}): {id}",
            record.snapshot.status
        );
    }
    record.inbox.push(SubagentInboxMessage {
        from: from.to_string(),
        text: text.to_string(),
        queued_at: unix_seconds(),
        id: meta.id,
        reply_to: meta.reply_to,
        from_addr: meta.from_addr,
    });
    // 入队即记入时间线：TUI 子智能体视图与 Web 详情立即可见，
    // 不必等到消息真正注入对话的步间间隙
    record.timeline.push_message(from, text);
    record.snapshot.pending_messages = record.inbox.len();
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let snapshot = record.snapshot.clone();
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
    Ok(snapshot)
}

/// 向指定父会话中的子智能体投递追加消息。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `id`: 子智能体 ID
/// - `from`: 消息来源标识
/// - `text`: 消息正文
///
/// 返回:
/// - 入队后的子智能体快照；子智能体不属于该会话时报错
pub(crate) fn queue_subagent_message_for_owner(
    owner_key: &str,
    id: &str,
    from: &str,
    text: &str,
) -> Result<SubagentSnapshot> {
    ensure_owner_belongs(owner_key, id)?;
    queue_subagent_message(id, from, text)
}

/// 取出子智能体的全部待注入消息并转入历史记录。
///
/// 供子智能体运行循环在每次请求模型前调用；队列为空时不产生
/// 任何状态变更，避免步间高频轮询带来无谓的持久化开销。
///
/// 参数:
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 按入队顺序排列的消息；记录不存在或队列为空时为空
pub(crate) fn drain_subagent_inbox(id: &str) -> Vec<SubagentInboxMessage> {
    let mut subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get_mut(id) else {
        return Vec::new();
    };
    if record.inbox.is_empty() {
        return Vec::new();
    }
    let messages = std::mem::take(&mut record.inbox);
    record.message_log.extend(messages.iter().cloned());
    record.snapshot.pending_messages = 0;
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
    messages
}

/// 查询子智能体队列中待注入的消息数量。
///
/// 供 worker 的待命轮询使用，只读且不触发事件发布。
///
/// 参数:
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 待注入消息数量；记录不存在时为 0
pub(crate) fn subagent_inbox_len(id: &str) -> usize {
    subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .map(|record| record.inbox.len())
        .unwrap_or(0)
}

/// 读取子智能体收到过的全部消息（已注入历史 + 待注入队列）。
///
/// 参数:
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 按时间顺序排列的消息列表；记录不存在时为空
pub(crate) fn subagent_messages(id: &str) -> Vec<SubagentInboxMessage> {
    let subagents = subagents().lock().expect("subagent state lock");
    let Some(record) = subagents.get(id) else {
        return Vec::new();
    };
    let mut messages = record.message_log.clone();
    messages.extend(record.inbox.iter().cloned());
    messages
}

/// 请求指定父会话中的持久子智能体优雅结束。
///
/// idle 态的子智能体会立即收尾；running 态的会在当前任务段完成后收尾，
/// 不打断进行中的工具调用。收尾成功后按 `apply` 决定是否把 worktree
/// 变更 apply 回主工作区。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `id`: 子智能体 ID
/// - `apply`: 结束时是否 apply worktree 变更
///
/// 返回:
/// - 请求登记后的子智能体快照；非持久或已终态时报错
pub(crate) fn request_subagent_stop_for_owner(
    owner_key: &str,
    id: &str,
    apply: bool,
) -> Result<SubagentSnapshot> {
    ensure_owner_belongs(owner_key, id)?;
    let mut subagents = subagents().lock().expect("subagent state lock");
    let record = subagents
        .get_mut(id)
        .ok_or_else(|| anyhow::anyhow!("subagent not found: {id}"))?;
    if !record.snapshot.persistent {
        bail!("subagent is not persistent; use action=cancel to stop it: {id}");
    }
    if record.snapshot.status != "running" && record.snapshot.status != "idle" {
        bail!(
            "subagent already finished (status={}): {id}",
            record.snapshot.status
        );
    }
    record.stop_request = Some(SubagentStopRequest { apply });
    record.snapshot.updated_at = unix_seconds();
    publish_record(record);
    let snapshot = record.snapshot.clone();
    let owner_key = record.owner_key.clone();
    persist_owner_locked(&subagents, &owner_key);
    Ok(snapshot)
}

/// 查询子智能体是否收到优雅结束请求。
///
/// 参数:
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 已登记的结束请求；未请求或记录不存在时为空
pub(crate) fn subagent_stop_requested(id: &str) -> Option<SubagentStopRequest> {
    subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .and_then(|record| record.stop_request)
}

/// 校验子智能体属于指定父会话。
///
/// 参数:
/// - `owner_key`: 父会话稳定作用域键
/// - `id`: 子智能体 ID
///
/// 返回:
/// - 属于该会话时返回 Ok
fn ensure_owner_belongs(owner_key: &str, id: &str) -> Result<()> {
    ensure_owner_loaded(owner_key);
    let belongs = subagents()
        .lock()
        .expect("subagent state lock")
        .get(id)
        .is_some_and(|record| record.owner_key == owner_key);
    if !belongs {
        bail!("subagent not found in current session: {id}");
    }
    Ok(())
}

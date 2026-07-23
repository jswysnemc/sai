use super::*;

/// 将记录当前状态写入详情事件流。
///
/// 参数:
/// - `record`: 子智能体运行记录
pub(super) fn publish_record(record: &mut SubagentRecord) {
    record
        .event_journal
        .publish(record.snapshot.clone(), record.timeline.entries());
}

/// 首次访问父会话时恢复其持久化子智能体记录。
pub(super) fn ensure_owner_loaded(owner_key: &str) {
    let mut loaded = loaded_owners().lock().expect("subagent owner lock");
    if !loaded.insert(owner_key.to_string()) {
        return;
    }
    let Ok(records) = subagent_persistence::load(owner_key) else {
        return;
    };
    let mut subagents = subagents().lock().expect("subagent state lock");
    for persisted in records {
        if subagents.contains_key(&persisted.snapshot.id) {
            continue;
        }
        let mut snapshot = persisted.snapshot;
        if snapshot.status == "running" {
            snapshot.status = "failed".to_string();
            snapshot.error = Some("子智能体进程在完成前中断".to_string());
            snapshot.updated_at = unix_seconds();
        }
        let mut record = SubagentRecord {
            owner_key: persisted.owner_key,
            snapshot,
            cancel: None,
            finish_notified: persisted.finish_notified,
            timeline: SubagentTimeline::from_entries(persisted.timeline),
            event_journal: SubagentEventJournal::new(),
        };
        publish_record(&mut record);
        subagents.insert(record.snapshot.id.clone(), record);
    }
    persist_owner_locked(&subagents, owner_key);
}

/// 保存指定父会话当前全部子智能体记录。
pub(super) fn persist_owner_locked(subagents: &HashMap<String, SubagentRecord>, owner_key: &str) {
    let records = subagents
        .values()
        .filter(|record| record.owner_key == owner_key)
        .map(|record| PersistedSubagent {
            owner_key: record.owner_key.clone(),
            snapshot: record.snapshot.clone(),
            timeline: record.timeline.entries(),
            finish_notified: record.finish_notified,
        })
        .collect::<Vec<_>>();
    let _ = subagent_persistence::save(owner_key, &records);
}

/// 获取当前 Unix 秒数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Unix 秒数
pub(super) fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// 获取后台子智能体表。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 全局子智能体表
pub(super) fn subagents() -> &'static Mutex<HashMap<String, SubagentRecord>> {
    SUBAGENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 获取已完成持久化恢复的父会话集合。
fn loaded_owners() -> &'static Mutex<HashSet<String>> {
    LOADED_OWNERS.get_or_init(|| Mutex::new(HashSet::new()))
}

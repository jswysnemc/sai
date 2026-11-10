use super::*;
use crate::tools::subagent_persistence::SubagentExecutionContext;
use crate::tools::subagent_runner::checkpoint::SubagentCheckpoint;

/// 【子任务】【重启测试】丢弃进程内缓存，后续访问必须重新读取磁盘。
/// 参数：会话作用域；返回无。
fn simulate_restart(owner: &str) {
    subagents()
        .lock()
        .unwrap()
        .retain(|_, record| record.owner_key != owner);
    LOADED_OWNERS.get().unwrap().lock().unwrap().remove(owner);
}

/// 【子任务】【重启回归】重新投递未进入对话检查点的消息，保留已消费消息且禁止重复认领。
#[test]
fn restores_inbox_consumption_and_claims_interrupted_worker_once() {
    let root = tempfile::tempdir().unwrap();
    let owner = root.path().display().to_string();
    let (snapshot, _cancel) =
        create_subagent_for_owner_goal(&owner, None, "restore".into(), "general".into(), 5, true);
    let id = &snapshot.id;
    set_execution_context(
        id,
        SubagentExecutionContext {
            cwd: root.path().to_path_buf(),
            worktree: None,
        },
    )
    .unwrap();
    queue_subagent_message(id, "user", "committed").unwrap();
    drain_subagent_inbox(id);
    save_checkpoint(id, SubagentCheckpoint::default()).unwrap();
    queue_subagent_message(id, "user", "consumed before crash").unwrap();
    drain_subagent_inbox(id);
    queue_subagent_message(id, "user", "still queued").unwrap();
    simulate_restart(&owner);
    let snapshot = subagent_snapshot_for_owner(&owner, id).unwrap();
    assert_eq!(snapshot.status, "interrupted");
    assert_eq!(snapshot.pending_messages, 2);
    assert_eq!(subagent_message_counts(id), (3, 1));
    let messages = subagent_messages(id);
    assert_eq!(
        messages
            .iter()
            .map(|message| message.text.as_str())
            .collect::<Vec<_>>(),
        vec!["committed", "consumed before crash", "still queued"]
    );
    assert!(prepare_resume("other-owner", id).is_err());
    let (_, _, _cancel) = prepare_resume(&owner, id).unwrap();
    assert!(prepare_resume(&owner, id).is_err());
    assert_eq!(drain_subagent_inbox(id).len(), 2);
}

/// 【子任务】【落盘失败】检查点写入错误必须传播给执行器，不得继续执行副作用。
#[test]
fn checkpoint_write_failure_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let owner_path = root.path().join("owner");
    let owner = owner_path.display().to_string();
    let (snapshot, _cancel) =
        create_subagent_for_owner(&owner, "disk error".into(), "general".into(), 1);
    std::fs::remove_dir_all(&owner_path).unwrap();
    std::fs::write(&owner_path, "not a directory").unwrap();
    assert!(save_checkpoint(&snapshot.id, SubagentCheckpoint::default()).is_err());
}

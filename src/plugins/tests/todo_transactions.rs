use super::{
    todo_support::{call, context, installed, item, record_path, seed, snapshot},
    todo_transaction_support::{runtime, TransactionHost},
};
use crate::state::StateStore;
use serde_json::json;
use std::{sync::atomic::Ordering, time::Duration};

/// 【待办事务测试】【只读快照】对象字段顺序不同不能触发一次没有业务变化的发布
/// @returns 无；已有合法状态的重复查询不进入比较交换
#[tokio::test]
async fn todo_snapshot_does_not_republish_equivalent_json_objects() {
    let root = tempfile::tempdir().unwrap();
    let (paths, _, inner) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let bytes = json!({"version":1,"items":[item("a","pending")],"history":[]}).to_string();
    std::fs::write(record_path(&paths, &scope), &bytes).unwrap();
    let host = TransactionHost::new(inner);
    for _ in 0..4 {
        let plugin = runtime(host.clone());
        snapshot(&plugin, root.path(), &scope).await;
    }
    assert_eq!(host.exchanges.load(Ordering::SeqCst), 0);
    assert_eq!(
        std::fs::read_to_string(record_path(&paths, &scope)).unwrap(),
        bytes
    );
}

/// 【待办事务测试】【序号目标绑定】竞争插入首项后仍修改首次选定的条目
/// @returns 无；更新和删除均不能误操作新的相同序号
#[tokio::test]
async fn todo_cas_retry_keeps_the_original_index_target() {
    for action in ["update", "remove"] {
        let root = tempfile::tempdir().unwrap();
        let (paths, _, inner) = installed(root.path());
        let store = StateStore::new(&paths).unwrap();
        let scope = store.state_dir().display().to_string();
        seed(
            &paths,
            &scope,
            json!([item("a", "pending"), item("b", "pending")]),
            json!([]),
        );
        let host = TransactionHost::new(inner);
        *host.replacement.lock().unwrap() = Some(
            json!({"version":1,"items":[item("inserted","pending"),item("a","pending"),item("b","pending")],"history":[]}),
        );
        let plugin = runtime(host.clone());
        let result = call(
            &plugin,
            root.path(),
            &scope,
            json!({"action":action,"index":1,"text":"updated"}),
        )
        .await;
        assert_eq!(result["changed"][0]["id"], "a");
        assert_eq!(result["items"][0]["id"], "inserted");
        assert_eq!(result["items"][0]["text"], "item inserted");
        assert_eq!(host.exchanges.load(Ordering::SeqCst), 2);
    }
}

/// 【待办事务测试】【目标已移除】竞争删除原目标后返回定位错误，保留后继条目
/// @returns 无；不能依据过期序号删除下一项
#[tokio::test]
async fn todo_cas_retry_rejects_a_removed_original_target() {
    let root = tempfile::tempdir().unwrap();
    let (paths, _, inner) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    seed(
        &paths,
        &scope,
        json!([item("a", "pending"), item("b", "pending")]),
        json!([]),
    );
    let host = TransactionHost::new(inner);
    *host.replacement.lock().unwrap() =
        Some(json!({"version":1,"items":[item("b","pending")],"history":[]}));
    let plugin = runtime(host);
    assert!(plugin
        .call_tool(
            "todo",
            json!({"action":"remove","index":1}),
            context(root.path(), &scope, true)
        )
        .await
        .is_err());
    assert_eq!(
        snapshot(&plugin, root.path(), &scope).await["items"],
        json!([item("b", "pending")])
    );
}

/// 【待办事务测试】【提交失败】重试耗尽或磁盘发布错误不留下部分活动状态和历史
/// @returns 无；故障解除后同一实例可以完整完成归档
#[tokio::test]
async fn todo_cas_exhaustion_and_publication_failure_are_atomic() {
    let root = tempfile::tempdir().unwrap();
    let (paths, _, inner) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let legacy = json!({"version":1,"items":[item("a", "pending")],"history":[]}).to_string();
    std::fs::write(record_path(&paths, &scope), &legacy).unwrap();
    let host = TransactionHost::new(inner);
    let plugin = runtime(host.clone());
    let args = json!({"action":"update","index":1,"status":"completed"});
    host.conflicts.store(16, Ordering::SeqCst);
    let error = plugin
        .call_tool("todo", args.clone(), context(root.path(), &scope, true))
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("changed too often"),
        "{error:#}"
    );
    assert_eq!(host.exchanges.load(Ordering::SeqCst), 16);
    host.fail.store(1, Ordering::SeqCst);
    let error = plugin
        .call_tool("todo", args.clone(), context(root.path(), &scope, true))
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("publication failure"),
        "{error:#}"
    );
    assert!(record_path(&paths, &scope).exists());
    assert_eq!(
        std::fs::read_to_string(record_path(&paths, &scope)).unwrap(),
        legacy
    );
    host.fail.store(0, Ordering::SeqCst);
    call(&plugin, root.path(), &scope, args).await;
    let state = snapshot(&plugin, root.path(), &scope).await;
    assert_eq!(state["items"], json!([]));
    assert_eq!(state["history"].as_array().unwrap().len(), 1);
}

/// 【待办事务测试】【发布后取消】外层取消不能回滚已提交文件，历史和活动项保持完整
/// @returns 无；恢复后不会再次归档，也不遗留虚拟机锁
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn todo_cancellation_after_publication_keeps_one_complete_archive() {
    let root = tempfile::tempdir().unwrap();
    let (paths, _, inner) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    seed(&paths, &scope, json!([item("a", "pending")]), json!([]));
    let host = TransactionHost::new(inner);
    let plugin = runtime(host.clone());
    *host.pause.lock().unwrap() = true;
    let running = plugin.clone();
    let ctx = context(root.path(), &scope, true);
    let task = tokio::spawn(async move {
        running
            .call_tool(
                "todo",
                json!({"action":"update","index":1,"status":"completed"}),
                ctx,
            )
            .await
    });
    let entered = tokio::time::timeout(Duration::from_secs(2), host.published.notified()).await;
    if entered.is_err() {
        host.resume();
    }
    entered.unwrap();
    task.abort();
    let cancelled = task.await.unwrap_err().is_cancelled();
    host.resume();
    assert!(cancelled);
    let saved = tokio::time::timeout(
        Duration::from_secs(2),
        snapshot(&plugin, root.path(), &scope),
    )
    .await
    .unwrap();
    assert_eq!(saved["items"], json!([]));
    assert_eq!(saved["history"].as_array().unwrap().len(), 1);
    assert_eq!(saved["history"][0]["items"][0]["status"], "completed");
}

use super::todo_support::{bundled, call, item, snapshot};
use crate::{config::AppConfig, plugins::todo_view::TodoView, state::StateStore};
use sai_plugin_runtime::host::{PluginHost, StorageRequest};
use serde_json::{json, Value};

/// 【待办兼容测试】【旧记录接续】完整接续活动项及历史，清空对话保留计划且旧文件字节不变
/// @returns 无；新状态和归档通过一个固定会话文件发布
#[tokio::test]
async fn todo_legacy_records_survive_conversation_reset() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = bundled(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let old = serde_json::to_vec(&json!([item("a", "pending"), item("b", "pending")])).unwrap();
    let history = json!([{"archived_at":"old-archive","items":[item("finished","completed")]}]);
    std::fs::write(store.state_dir().join("todos.json"), &old).unwrap();
    std::fs::write(
        store.state_dir().join("todos.history.json"),
        history.to_string(),
    )
    .unwrap();
    let initial = snapshot(&runtime, root.path(), &scope).await;
    assert_eq!(initial["items"].as_array().unwrap().len(), 2);
    assert_eq!(initial["history"], history);
    call(
        &runtime,
        root.path(),
        &scope,
        json!({"action":"update","index":1,"status":"completed"}),
    )
    .await;
    let changed = snapshot(&runtime, root.path(), &scope).await;
    assert_eq!(changed["items"][0]["status"], "completed");
    store.reset_conversation().unwrap();
    assert_eq!(snapshot(&runtime, root.path(), &scope).await, changed);
    assert_eq!(
        std::fs::read(store.state_dir().join("todos.json")).unwrap(),
        old
    );
    assert_eq!(
        std::fs::read_to_string(store.state_dir().join("todos.history.json")).unwrap(),
        history.to_string()
    );
    assert!(store.state_dir().join("todos.plugin.json").is_file());
}

/// 【待办兼容测试】【界面完成归档】Web 使用实际 Lua 收敛旧完成计划，重复读取不增加历史
/// @returns 无；保留旧 Web 字段与归档顺序
#[tokio::test]
async fn todo_web_view_archives_legacy_completion_once() {
    let root = tempfile::tempdir().unwrap();
    let (paths, _, _) = bundled(root.path());
    let store = StateStore::new(&paths).unwrap();
    std::fs::write(
        store.state_dir().join("todos.json"),
        json!([item("done", "completed")]).to_string(),
    )
    .unwrap();
    let view = TodoView::load(&AppConfig::default(), &paths).await.unwrap();
    let first = serde_json::to_value(
        view.snapshot(store.session_id(), store.state_dir(), root.path())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first["items"], json!([]));
    assert_eq!(first["history"].as_array().unwrap().len(), 1);
    assert_eq!(first["history"][0]["items"][0]["created_at"], "old-created");
    let second = serde_json::to_value(
        view.snapshot(store.session_id(), store.state_dir(), root.path())
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first, second);
}

/// 【待办兼容测试】【墓碑与作用域】显式删除不能重新导入旧记录，伪造目录及无授权读取均拒绝
/// @returns 无；直接工具作用域继续使用普通私有记录
#[tokio::test]
async fn todo_legacy_scope_and_tombstone_are_authoritative() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, host) = bundled(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    std::fs::write(
        store.state_dir().join("todos.json"),
        json!([item("old", "pending")]).to_string(),
    )
    .unwrap();
    let caps = runtime.manifest().capabilities.clone();
    host.storage(
        StorageRequest::Set {
            key: "plan".into(),
            value: Value::Null,
        },
        &scope,
        &caps,
    )
    .unwrap();
    assert_eq!(
        host.storage(StorageRequest::Get { key: "plan".into() }, &scope, &caps)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await,
        json!({"items":[],"history":[]})
    );
    assert!(host
        .storage(
            StorageRequest::Get { key: "plan".into() },
            &root.path().display().to_string(),
            &caps
        )
        .is_err());
    assert!(host
        .storage(
            StorageRequest::Get { key: "plan".into() },
            &scope,
            &Default::default()
        )
        .is_err());
    assert_eq!(
        host.storage(
            StorageRequest::Get { key: "plan".into() },
            "cli-tool",
            &caps
        )
        .unwrap(),
        Value::Null
    );
    let external = crate::plugins::private::PrivatePluginHost::new(&paths, "todo");
    assert_eq!(
        external
            .storage(StorageRequest::Get { key: "plan".into() }, &scope, &caps)
            .unwrap(),
        Value::Null
    );
}

/// 【待办兼容测试】【损坏数据】非法旧 JSON 或条目不能覆盖原文件，也不创建新记录
/// @returns 无；后续修正源文件后同一实例可以继续查询
#[tokio::test]
async fn todo_legacy_corruption_is_not_silently_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let (paths, runtime, _) = bundled(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let ctx = super::todo_support::context(root.path(), &scope, true);
    for text in ["{", "{}", "[{}]"] {
        std::fs::write(store.state_dir().join("todos.json"), text).unwrap();
        assert!(runtime
            .call_tool("todo", json!({"action":"add","text":"new"}), ctx.clone())
            .await
            .is_err());
        assert_eq!(
            std::fs::read_to_string(store.state_dir().join("todos.json")).unwrap(),
            text
        );
        assert!(!store.state_dir().join("todos.plugin.json").exists());
    }
    std::fs::write(store.state_dir().join("todos.json"), " \n\t").unwrap();
    assert_eq!(
        snapshot(&runtime, root.path(), &scope).await["items"],
        json!([])
    );
}

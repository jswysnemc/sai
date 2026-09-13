use super::todo_support::{context, installed, snapshot};
use crate::state::StateStore;
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::time::Duration;

/// 【待办并发测试】【忙锁重试】仅重试明确没有提交的锁占用，其他错误立即返回
/// @param plugin 独立实例；args 为完整参数；ctx 为可信上下文
/// @returns 工具原结果或实际错误
async fn call_with_busy_retry(
    plugin: &PluginRuntime,
    args: Value,
    ctx: InvocationContext,
) -> anyhow::Result<Value> {
    for _ in 0..128 {
        match plugin.call_tool("todo", args.clone(), ctx.clone()).await {
            Ok(text) => return Ok(serde_json::from_str(&text)?),
            Err(error) if format!("{error:#}").contains("private data is busy") => {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
            Err(error) => return Err(error),
        }
    }
    anyhow::bail!("busy lock retry budget exhausted")
}

/// 【待办并发测试】【新增合并】八个独立实例并发新增，不能丢失任何清单项
/// @returns 无；保存数量、唯一标识和文字与成功调用一致
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn todo_concurrent_additions_keep_all_successful_items() {
    let root = tempfile::tempdir().unwrap();
    let (paths, reader, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let mut tasks = Vec::new();
    for number in 0..8 {
        let (_, plugin, _) = installed(root.path());
        let ctx = context(root.path(), &scope, true);
        tasks.push(tokio::spawn(async move {
            call_with_busy_retry(
                &plugin,
                json!({"action":"add","text":format!("item {number}")}),
                ctx,
            )
            .await
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap()["ok"], true);
    }
    let state = snapshot(&reader, root.path(), &scope).await;
    let items = state["items"].as_array().unwrap();
    assert_eq!(items.len(), 8);
    let ids = items
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    let texts = items
        .iter()
        .map(|item| item["text"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(ids.len(), 8);
    assert_eq!(texts.len(), 8);
}

/// 【待办并发测试】【字段合并】相同目标的文字与状态同时更新，两者都必须保留
/// @returns 无；完成最后一项时只产生一份完整归档
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn todo_concurrent_updates_merge_and_final_archive_is_unique() {
    let root = tempfile::tempdir().unwrap();
    let (paths, reader, _) = installed(root.path());
    let store = StateStore::new(&paths).unwrap();
    let scope = store.state_dir().display().to_string();
    let added = call_with_busy_retry(
        &reader,
        json!({"action":"add","texts":["first","last"]}),
        context(root.path(), &scope, true),
    )
    .await
    .unwrap();
    let id = added["items"][0]["id"].clone();
    let mut tasks = Vec::new();
    for update in [
        json!({"action":"update","id":id,"text":"changed"}),
        json!({"action":"update","id":id,"status":"completed"}),
    ] {
        let (_, plugin, _) = installed(root.path());
        let ctx = context(root.path(), &scope, true);
        tasks.push(tokio::spawn(async move {
            call_with_busy_retry(&plugin, update, ctx).await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let state = snapshot(&reader, root.path(), &scope).await;
    assert_eq!(state["items"][0]["text"], "changed");
    assert_eq!(state["items"][0]["status"], "completed");
    let last = state["items"][1]["id"].clone();
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let (_, plugin, _) = installed(root.path());
        let ctx = context(root.path(), &scope, true);
        let update = json!({"action":"update","id":last,"status":"completed"});
        tasks.push(tokio::spawn(async move {
            call_with_busy_retry(&plugin, update, ctx).await
        }));
    }
    let mut successes = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => successes += 1,
            Err(error) => assert!(format!("{error:#}").contains("not found"), "{error:#}"),
        }
    }
    assert_eq!(successes, 1);
    let state = snapshot(&reader, root.path(), &scope).await;
    assert_eq!(state["items"], json!([]));
    assert_eq!(state["history"].as_array().unwrap().len(), 1);
    assert_eq!(state["history"][0]["items"].as_array().unwrap().len(), 2);
}

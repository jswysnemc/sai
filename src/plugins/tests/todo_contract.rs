use super::todo_support::{call, context, package, runtime, snapshot};
use serde_json::json;

/// 【待办测试】【完整计划】批量新增、插入、顺序推进及取消最终只归档一次
/// @returns 无；变更项与完整活动快照保持原接口
#[tokio::test]
async fn todo_lua_preserves_ordered_plan_and_archives_once() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path());
    let first = call(
        &plugin,
        root.path(),
        "one",
        json!({"action":"add","texts":[" first ","third"]}),
    )
    .await;
    assert_eq!(first["changed"].as_array().unwrap().len(), 2);
    let added = call(
        &plugin,
        root.path(),
        "one",
        json!({"action":"add","text":"second","index":2}),
    )
    .await;
    assert_eq!(added["items"][1]["text"], "second");
    let failure = plugin
        .call_tool(
            "todo",
            json!({"action":"update","index":2,"status":"in_progress"}),
            context(root.path(), "one", true),
        )
        .await
        .unwrap_err();
    assert!(format!("{failure:#}").contains("update index 1 first"));
    call(
        &plugin,
        root.path(),
        "one",
        json!({"action":"update","index":1,"status":"completed"}),
    )
    .await;
    call(
        &plugin,
        root.path(),
        "one",
        json!({"action":"update","index":2,"status":"cancelled"}),
    )
    .await;
    let finished = call(
        &plugin,
        root.path(),
        "one",
        json!({"action":"update","index":3,"status":"completed"}),
    )
    .await;
    assert_eq!(finished["changed"][0]["status"], "completed");
    assert_eq!(finished["items"], json!([]));
    let archived = snapshot(&plugin, root.path(), "one").await;
    assert_eq!(archived["history"].as_array().unwrap().len(), 1);
    assert_eq!(archived["history"][0]["items"].as_array().unwrap().len(), 3);
    assert_eq!(snapshot(&plugin, root.path(), "one").await, archived);
    assert_eq!(
        snapshot(&plugin, root.path(), "two").await,
        json!({"items":[],"history":[]})
    );
}

/// 【待办测试】【重载与失败原子性】新实例延续原会话，非法更新和只读调用不改记录
/// @returns 无；写入工具保持原有权限边界
#[tokio::test]
async fn todo_lua_persists_and_rejects_invalid_mutations() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path());
    call(
        &plugin,
        root.path(),
        "one",
        json!({"action":"add","text":"keep"}),
    )
    .await;
    let before = snapshot(&plugin, root.path(), "one").await;
    for args in [
        json!({"action":"add","texts":["good", " "]}),
        json!({"action":"update","index":1,"text":" "}),
        json!({"action":"remove","index":0}),
        json!({"action":"update","index":1}),
    ] {
        assert!(plugin
            .call_tool("todo", args, context(root.path(), "one", true))
            .await
            .is_err());
        assert_eq!(snapshot(&plugin, root.path(), "one").await, before);
    }
    assert!(plugin
        .call_tool(
            "todo",
            json!({"action":"add","text":"blocked"}),
            context(root.path(), "one", false)
        )
        .await
        .is_err());
    assert_eq!(
        snapshot(&runtime(root.path()), root.path(), "one").await,
        before
    );
    assert!(package().manifest.capabilities.system.session_storage);
    assert!(package().manifest.capabilities.reply_policy);
}

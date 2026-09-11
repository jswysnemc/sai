use super::todo_support::{context, runtime};
use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    host::{PluginHost, StorageRequest},
    ToolPolicyInput,
};
use serde_json::{json, Value};

/// 【待办提醒测试】【原版序列】比较连续计数、更新重置、计划结束和单循环最多一次提醒
/// @returns 无；127 条固定样本逐项使用实际 Lua 回调
#[tokio::test]
async fn todo_lua_matches_native_reminder_sequences() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path());
    let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "todo");
    let caps = plugin.manifest().capabilities.clone();
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/todo_reminders_01.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 127);
    for case in fixture["cases"].as_array().unwrap() {
        let session = case["label"].as_str().unwrap();
        host.storage(
            StorageRequest::Set {
                key: "plan".into(),
                value: json!({"version":1,"items":case["items"],"history":[]}),
            },
            session,
            &caps,
        )
        .unwrap();
        let mut state = Value::Null;
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            if let Some(items) = step.get("items") {
                host.storage(
                    StorageRequest::Set {
                        key: "plan".into(),
                        value: json!({"version":1,"items":items,"history":[]}),
                    },
                    session,
                    &caps,
                )
                .unwrap();
            }
            let updated = step["updated"].as_bool().unwrap();
            let input = ToolPolicyInput {
                name: if updated { "todo" } else { "fixture" }.into(),
                local_name: updated.then(|| "todo".into()),
                arguments: if updated {
                    json!({"action":"update"})
                } else {
                    json!({})
                },
                ok: true,
                tools: vec!["todo".into()],
            };
            let output = plugin
                .after_tool(input, state, context(root.path(), session, false))
                .await
                .unwrap();
            assert_eq!(
                json!(output.reminder),
                case["expected"][index]["value"],
                "{session} step {index}"
            );
            state = output.state;
        }
    }
}

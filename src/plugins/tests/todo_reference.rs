use super::todo_support::{context, package, runtime, snapshot};
use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
use sai_plugin_runtime::{
    host::{PluginHost, StorageRequest},
    PluginRuntime,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc};

/// 【待办对照测试】【非确定字段】只规范动态标识和时钟，保留所有业务字段与数组顺序
/// @param value 原结果；identifiers 为本样本已经遇到的标识
/// @returns 可与冻结原版比较的完整结果
pub(super) fn normalize(value: &mut Value, identifiers: &mut BTreeMap<String, String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                normalize(item, identifiers);
            }
        }
        Value::Object(fields) => {
            let mut keys: Vec<_> = fields.keys().cloned().collect();
            keys.sort();
            for key in keys {
                let item = fields.get_mut(&key).unwrap();
                let text = item.as_str().map(str::to_string);
                if key == "id"
                    && text.as_deref().is_some_and(|text| {
                        let parts: Vec<_> = text.split('_').collect();
                        parts.len() == 4
                            && parts[0] == "todo"
                            && parts[1..].iter().all(|part| {
                                !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())
                            })
                    })
                {
                    let text = text.unwrap();
                    let fallback = format!("$id{}", identifiers.len() + 1);
                    *item = json!(identifiers.entry(text).or_insert(fallback));
                } else if matches!(key.as_str(), "created_at" | "updated_at" | "archived_at")
                    && text
                        .as_deref()
                        .is_some_and(|text| chrono::DateTime::parse_from_rfc3339(text).is_ok())
                {
                    *item = json!("$time");
                } else {
                    normalize(item, identifiers);
                }
            }
        }
        _ => {}
    }
}

/// 【待办对照测试】【完整业务矩阵】执行实际 Lua，对比冻结原版的结果、错误、清单及归档
/// @returns 无；违反公开 Schema 的旧宽容参数明确归入拒绝测试
#[tokio::test]
async fn todo_lua_matches_native_business_matrix() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path());
    let host = PrivatePluginHost::new(&SaiPaths::for_tests(root.path()), "todo");
    let caps = plugin.manifest().capabilities.clone();
    let mut count = 0;
    for source in [
        include_str!("fixtures/todo_reference_01.json"),
        include_str!("fixtures/todo_reference_02.json"),
        include_str!("fixtures/todo_reference_03.json"),
        include_str!("fixtures/todo_reference_04.json"),
    ] {
        let fixture: Value = serde_json::from_str(source).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            count += 1;
            let session = case["label"].as_str().unwrap();
            let seeded = json!({"version":1,"items":case["items"],"history":case.get("history").cloned().unwrap_or(json!([]))});
            host.storage(
                StorageRequest::Set {
                    key: "plan".into(),
                    value: seeded.clone(),
                },
                session,
                &caps,
            )
            .unwrap();
            let mut identifiers = BTreeMap::new();
            for (index, args) in case["calls"].as_array().unwrap().iter().enumerate() {
                let result = plugin
                    .call_tool("todo", args.clone(), context(root.path(), session, true))
                    .await;
                if case["schema_valid"] == false {
                    let error = result.unwrap_err();
                    assert!(
                        format!("{error:#}").contains("arguments"),
                        "{session}: {error:#}"
                    );
                    assert_eq!(
                        host.storage(StorageRequest::Get { key: "plan".into() }, session, &caps)
                            .unwrap(),
                        seeded
                    );
                    continue;
                }
                let expected = &case["expected"][index];
                let actual_call = if expected["call"]["ok"] == true {
                    let result = result.unwrap_or_else(|error| panic!("{session}: {error:#}"));
                    json!({"ok":true,"result":serde_json::from_str::<Value>(&result).unwrap()})
                } else {
                    let error = result.unwrap_err();
                    let message = expected["call"]["error"].as_str().unwrap();
                    assert!(
                        format!("{error:#}").contains(message),
                        "{session}: expected {message}; got {error:#}"
                    );
                    json!({"ok":false,"error":message})
                };
                let state = snapshot(&plugin, root.path(), session).await;
                let mut actual =
                    json!({"call":actual_call,"items":state["items"],"history":state["history"]});
                normalize(&mut actual, &mut identifiers);
                assert_eq!(&actual, expected, "{session} step {index}");
            }
        }
    }
    assert_eq!(count, 989);
}

/// 【待办对照测试】【双语契约】逐字段比较原注册入口输出的名称、说明、参数和写入权限
/// @returns 无；没有增加或删除公开工具参数
#[test]
fn todo_lua_preserves_both_native_tool_definitions() {
    let root = tempfile::tempdir().unwrap();
    let fixtures: Value =
        serde_json::from_str(include_str!("fixtures/todo_definitions.json")).unwrap();
    for language in ["en", "zh"] {
        let package = package();
        let caps = package.manifest.capabilities.clone();
        let plugin = PluginRuntime::load(
            package,
            json!({"language":language}),
            caps,
            Arc::new(PrivatePluginHost::new(
                &SaiPaths::for_tests(root.path()),
                "todo",
            )),
        )
        .unwrap();
        assert_eq!(plugin.tools().len(), 1);
        let tool = &plugin.tools()[0];
        let expected = &fixtures[language];
        assert_eq!(tool.name, expected["name"].as_str().unwrap());
        assert_eq!(tool.description, expected["description"].as_str().unwrap());
        assert_eq!(tool.parameters, expected["parameters"]);
        assert_eq!(serde_json::to_value(tool.access).unwrap(), "writes");
    }
}

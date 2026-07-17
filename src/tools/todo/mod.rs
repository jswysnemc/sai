mod reminder;
mod store;

use super::{ToolRegistry, ToolSpec};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

pub(crate) use reminder::TodoReminder;
pub(crate) use store::{TodoItem, TodoStatus, TodoStore};

/// 注册会话级 TODO 工具。
///
/// 参数:
/// - `registry`: 当前交互式会话工具注册表
/// - `file`: 当前会话 TODO 状态文件
///
/// 返回:
/// - 无
pub(crate) fn register(registry: &mut ToolRegistry, file: PathBuf) {
    registry.register(
        ToolSpec::new(
            "todo",
            t(
                "Track a multi-step plan for the current session. Actions: list reads all items with 1-based indexes; add creates one item (text) or several at once (texts array), optionally inserting before position index; update changes an item's text or status; remove deletes an item. update and remove locate the item by id or by 1-based index. Status flows pending -> in_progress -> completed (or cancelled). Items advance in order: finish earlier items before advancing later ones and keep at most one in_progress; completing a pending item directly is allowed once all earlier items are finished. When every item is completed or cancelled, the plan is archived to history and the active list is cleared so the next plan starts fresh. Every mutating call returns the full active items snapshot, so you rarely need a separate list call. Prefer creating the whole plan in a single add call with texts. Use this for tasks with three or more steps; skip it for trivial single-step work.",
                "跟踪当前会话的多步计划。动作：list 读取全部条目并附 1 起始序号；add 创建单条(text)或一次创建多条(texts 数组)，可用 index 指定插入位置；update 修改条目文本或状态；remove 删除条目。update 与 remove 通过 id 或 1 起始的 index 定位条目。状态流转为 pending -> in_progress -> completed（或 cancelled）。条目按顺序推进：先完成前面的条目再推进后面的，同一时刻至多一个 in_progress；当前面条目全部完成时，允许把 pending 条目直接标记为 completed。当全部条目都 completed 或 cancelled 后，计划会归档到历史并清空活动列表，下一次计划从空白开始。每次修改都会返回完整活动清单快照，一般无需再单独调用 list。建议用一次 add 携带 texts 创建完整计划。任务达到三步及以上时使用，单步琐碎任务不必使用。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "add", "update", "remove"],
                        "description": t(
                            "Which operation to perform.",
                            "要执行的操作。",
                        )
                    },
                    "id": {
                        "type": "string",
                        "description": t(
                            "Item id for update or remove. Obtain it from any previous result; index is an alternative.",
                            "update 或 remove 的条目 id，可从任意先前结果获取；也可以改用 index 定位。",
                        )
                    },
                    "index": {
                        "type": "integer",
                        "description": t(
                            "1-based position. For add: insert before this position (defaults to appending). For update or remove: the target item when id is absent.",
                            "1 起始的序号。add 时表示插入到该位置之前（缺省追加到末尾）；update 或 remove 未提供 id 时用它定位目标条目。",
                        )
                    },
                    "text": {
                        "type": "string",
                        "description": t(
                            "Single item text. Required for add unless texts is given; optional for update to rename.",
                            "单条内容。add 时未提供 texts 则必填；update 时可选，用于改写文本。",
                        )
                    },
                    "texts": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": t(
                            "Multiple item texts for add, created in order in one call.",
                            "add 的批量内容，一次调用按顺序创建多条。",
                        )
                    },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "cancelled"],
                        "description": t(
                            "New status for update. Advance items in order and keep at most one in_progress.",
                            "update 的新状态。按顺序推进，同时至多一个 in_progress。",
                        )
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
            move |args| {
                let store = TodoStore::new(file.clone());
                async move { execute(args, store) }
            },
        )
        .writes(),
    );
}

/// 执行 TODO 工具动作。
///
/// 参数:
/// - `args`: 工具参数
/// - `store`: 当前会话 TODO 存储
///
/// 返回:
/// - JSON 格式执行结果,修改动作附带全量清单快照
fn execute(args: Value, store: TodoStore) -> Result<String> {
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let result = match action {
        "list" => json!({"ok": true, "items": store.list()?}),
        "add" => {
            let texts = collect_add_texts(&args)?;
            let changed = store.add_many(&texts, index_arg(&args))?;
            json!({"ok": true, "changed": changed, "items": store.list()?})
        }
        "update" => {
            let position = store.locate(optional_string(&args, "id"), index_arg(&args))?;
            let text = args.get("text").and_then(Value::as_str);
            let status = args
                .get("status")
                .and_then(Value::as_str)
                .map(TodoStatus::parse)
                .transpose()?;
            let changed = store.update_at(position, text, status)?;
            json!({"ok": true, "changed": [changed], "items": store.list()?})
        }
        "remove" => {
            let position = store.locate(optional_string(&args, "id"), index_arg(&args))?;
            let changed = store.remove_at(position)?;
            json!({"ok": true, "changed": [changed], "items": store.list()?})
        }
        _ => bail!("unsupported todo action: {action}"),
    };
    Ok(result.to_string())
}

/// 收集 add 动作的批量内容,texts 优先于 text。
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - 非空的内容列表
fn collect_add_texts(args: &Value) -> Result<Vec<String>> {
    if let Some(texts) = args.get("texts").and_then(Value::as_array) {
        let collected = texts
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .ok_or_else(|| anyhow::anyhow!("texts must be non-empty strings"))
            })
            .collect::<Result<Vec<_>>>()?;
        if !collected.is_empty() {
            return Ok(collected);
        }
    }
    Ok(vec![string_arg(args, "text")?.to_string()])
}

/// 读取可选的 1 起始序号参数。
///
/// 参数:
/// - `args`: 工具参数
///
/// 返回:
/// - 可选序号
fn index_arg(args: &Value) -> Option<usize> {
    args.get("index")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
}

/// 读取可选字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `name`: 参数名称
///
/// 返回:
/// - 非空可选字符串
fn optional_string<'value>(args: &'value Value, name: &str) -> Option<&'value str> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

/// 读取必填字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `name`: 参数名称
///
/// 返回:
/// - 非空字符串参数
fn string_arg<'value>(args: &'value Value, name: &str) -> Result<&'value str> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        bail!("{name} is required")
    }
    Ok(value)
}

/// 判断工具调用是否会修改 TODO 清单。
///
/// 参数:
/// - `arguments`: 工具调用 JSON 参数
///
/// 返回:
/// - 是否属于修改动作
pub(crate) fn is_mutating_call(arguments: &str) -> bool {
    serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|args| {
            args.get("action")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|action| matches!(action.as_str(), "add" | "update" | "remove"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证工具支持新增、更新、列表和删除。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn tool_supports_crud_actions() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = ToolRegistry::new();
        register(&mut registry, dir.path().join("todos.json"));

        let added = registry
            .call("todo", r#"{"action":"add","text":"first"}"#)
            .await
            .unwrap();
        let added = serde_json::from_str::<Value>(&added).unwrap();
        let id = added["changed"][0]["id"].as_str().unwrap().to_string();
        assert_eq!(added["items"].as_array().unwrap().len(), 1);
        registry
            .call(
                "todo",
                &json!({"action":"update","id":id,"status":"in_progress"}).to_string(),
            )
            .await
            .unwrap();
        let completed = registry
            .call(
                "todo",
                &json!({"action":"update","index":1,"status":"completed"}).to_string(),
            )
            .await
            .unwrap();
        let completed = serde_json::from_str::<Value>(&completed).unwrap();
        // 全部完成后归档，活动列表为空；变更快照仍应带回 completed 状态。
        assert_eq!(completed["changed"][0]["status"], "completed");
        assert_eq!(completed["items"], json!([]));
        let listed = registry.call("todo", r#"{"action":"list"}"#).await.unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&listed).unwrap()["items"],
            json!([])
        );
    }

    /// 验证一次调用可以批量创建条目并按序号插入。
    ///
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn tool_supports_batch_add_with_index() {
        let dir = tempfile::tempdir().unwrap();
        let mut registry = ToolRegistry::new();
        register(&mut registry, dir.path().join("todos.json"));

        let created = registry
            .call(
                "todo",
                &json!({"action":"add","texts":["one","three"]}).to_string(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&created).unwrap()["changed"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let inserted = registry
            .call(
                "todo",
                &json!({"action":"add","text":"two","index":2}).to_string(),
            )
            .await
            .unwrap();
        let items = serde_json::from_str::<Value>(&inserted).unwrap()["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["text"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(items, vec!["one", "two", "three"]);
    }
}

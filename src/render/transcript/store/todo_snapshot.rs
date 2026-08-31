use super::{TodoSnapshotItem, TranscriptStore};
use serde_json::Value;

impl TranscriptStore {
    /// 返回最近一次 todo 工具快照（供沉底面板展示）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 最新清单条目切片
    pub(crate) fn latest_todo_items(&self) -> &[TodoSnapshotItem] {
        &self.latest_todo
    }
}

/// 判断快照是否仍有未完成项。
///
/// 参数:
/// - `items`: todo 快照条目
///
/// 返回:
/// - 存在 pending 或 in_progress 时为 true
pub(super) fn snapshot_is_active(items: &[TodoSnapshotItem]) -> bool {
    items
        .iter()
        .any(|item| matches!(item.status.as_str(), "pending" | "in_progress"))
}

/// 从 todo 工具输出解析全量清单快照。
///
/// 参数:
/// - `output`: todo 工具结果 JSON（含 items 数组）
///
/// 返回:
/// - 解析成功时的条目列表
pub(super) fn parse_todo_snapshot(output: &str) -> Option<Vec<TodoSnapshotItem>> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    let items = value.get("items")?.as_array()?;
    Some(
        items
            .iter()
            .filter_map(|item| {
                let text = item.get("text")?.as_str()?.trim().to_string();
                if text.is_empty() {
                    return None;
                }
                let status = item
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("pending")
                    .to_string();
                Some(TodoSnapshotItem { status, text })
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_todo_snapshot, snapshot_is_active};
    use crate::render::transcript::TranscriptStore;

    #[test]
    fn parses_items_with_status() {
        let output = r#"{"ok":true,"items":[
            {"id":"a","text":"one","status":"completed"},
            {"id":"b","text":"two","status":"in_progress"},
            {"id":"c","text":"three","status":"pending"}
        ]}"#;
        let items = parse_todo_snapshot(output).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[1].status, "in_progress");
        assert_eq!(items[2].text, "three");
    }

    #[test]
    fn ignores_invalid_output() {
        assert!(parse_todo_snapshot("not json").is_none());
        assert!(parse_todo_snapshot(r#"{"ok":true}"#).is_none());
    }

    #[test]
    fn snapshot_is_active_only_when_unfinished_items_remain() {
        let pending =
            parse_todo_snapshot(r#"{"ok":true,"items":[{"text":"one","status":"pending"}]}"#)
                .unwrap();
        let completed = parse_todo_snapshot(
            r#"{"ok":true,"items":[{"text":"one","status":"completed"},{"text":"two","status":"cancelled"}]}"#,
        )
        .unwrap();
        assert!(snapshot_is_active(&pending));
        assert!(!snapshot_is_active(&completed));
    }

    #[test]
    fn completed_snapshot_does_not_stay_in_panel_state() {
        let mut store = TranscriptStore::new(10);
        store.push_tool_result(
            "todo".into(),
            true,
            r#"{"ok":true,"items":[{"text":"done","status":"completed"}]}"#.into(),
        );
        assert!(store.latest_todo_items().is_empty());
    }

    #[test]
    fn unfinished_snapshot_is_kept_until_clear() {
        let mut store = TranscriptStore::new(10);
        store.push_tool_result(
            "todo".into(),
            true,
            r#"{"ok":true,"items":[{"text":"next","status":"pending"}]}"#.into(),
        );
        assert_eq!(store.latest_todo_items().len(), 1);
        store.clear();
        assert!(store.latest_todo_items().is_empty());
    }
}

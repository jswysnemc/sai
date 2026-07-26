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
    use super::parse_todo_snapshot;

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
}

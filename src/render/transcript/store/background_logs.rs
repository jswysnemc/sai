use super::{HistoryCell, TranscriptStore};
use crate::render::transcript::tool_cell::ToolCell;
use serde_json::Value;

impl TranscriptStore {
    /// 返回仍运行的后台命令标识，无参数，返回去重后的任务列表。
    pub(crate) fn running_background_task_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for cell in &self.cells {
            if let HistoryCell::Tool(ToolCell::Invocation(view)) = cell {
                if view.is_running_background() {
                    if let Some(id) = view.background_task_id().filter(|id| !ids.contains(id)) {
                        ids.push(id);
                    }
                }
            }
        }
        ids
    }

    /// 判断是否有后台命令需要动效，无参数，返回活动标志。
    pub(crate) fn has_running_background_commands(&self) -> bool {
        !self.running_background_task_ids().is_empty()
    }

    /// 【终端】【后台日志】将可信日志快照合并到启动卡片，不改写历史读取卡片。
    ///
    /// 参数: `task_id` 为后台任务标识，`snapshot` 为运行期读取的状态及日志
    /// 返回: 是否更新了可见卡片
    pub(crate) fn update_background_logs(&mut self, task_id: &str, snapshot: &Value) -> bool {
        let mut first_changed = None;
        for (index, cell) in self.cells.iter_mut().enumerate() {
            let HistoryCell::Tool(ToolCell::Invocation(view)) = cell else {
                continue;
            };
            if view.background_task_id().as_deref() != Some(task_id) {
                continue;
            }
            let Some(outcome) = view.outcome.as_mut() else {
                continue;
            };
            let Ok(mut value) = serde_json::from_str::<Value>(&outcome.output) else {
                continue;
            };
            let Some(fields) = snapshot.as_object() else {
                continue;
            };
            for (key, field) in fields {
                if key == "task" {
                    if let (Some(current), Some(updated)) =
                        (value[key].as_object_mut(), field.as_object())
                    {
                        current.extend(updated.clone());
                        continue;
                    }
                }
                value[key] = field.clone();
            }
            let output = value.to_string();
            if output != outcome.output {
                outcome.output = output;
                first_changed.get_or_insert(index);
            }
        }
        if let Some(index) = first_changed {
            self.mark_dirty(index);
        }
        first_changed.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 启动卡片实时更新，历史日志卡保持原样；停止跟踪时保留最后日志和命令。
    #[test]
    fn background_updates_preserve_snapshots_and_stop_tracking_cleaned_tasks() {
        let mut store = TranscriptStore::new(200);
        let output = serde_json::json!({"task":{"id":"task-1","command":"build","status":"running"},"stdout":"first line"}).to_string();
        for action in ["start", "output"] {
            store.push_tool_call(
                "background_command".into(),
                serde_json::json!({"action":action}).to_string(),
            );
            store.push_tool_result("background_command".into(), true, output.clone());
        }
        assert_eq!(store.running_background_task_ids(), vec!["task-1"]);
        let snapshot = serde_json::json!({"stdout":"latest line","live_output":true});
        assert!(store.update_background_logs("task-1", &snapshot));
        assert!(!store.update_background_logs("task-1", &snapshot));
        assert!(store.update_background_logs(
            "task-1",
            &serde_json::json!({"task":{"status":"untracked"}})
        ));
        assert!(store.running_background_task_ids().is_empty());
        let values = store
            .cells
            .iter()
            .filter_map(|cell| match cell {
                HistoryCell::Tool(ToolCell::Invocation(view)) => {
                    serde_json::from_str::<Value>(&view.outcome.as_ref()?.output).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(values[0]["task"]["command"], "build");
        assert_eq!(values[0]["stdout"], "latest line");
        assert_eq!(values[1]["stdout"], "first line");
        assert_eq!(values[1]["task"]["status"], "running");
    }
}

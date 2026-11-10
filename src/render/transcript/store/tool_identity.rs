use super::*;

impl TranscriptStore {
    /// 【终端】【调用标识】为完整工具调用绑定稳定标识；参数为标识、名称和参数，返回无。
    pub(crate) fn push_identified_tool_call(&mut self, id: &str, name: String, arguments: String) {
        self.push_tool_call(name, arguments);
        if let Some(index) = self.active_tool_index {
            self.tool_call_ids.insert(id.to_string(), index);
        }
    }

    /// 【终端】【调用标识】按标识更新工具，缺失调用时创建独立载体，禁止匹配另一条同名调用。
    /// 参数：标识、名称及更新函数；返回更新函数结果。
    pub(crate) fn with_identified_tool<T>(
        &mut self,
        id: &str,
        name: &str,
        update: impl FnOnce(&mut Self) -> T,
    ) -> T {
        if !self.tool_call_ids.contains_key(id) {
            self.push_identified_tool_call(id, name.to_string(), String::new());
        }
        self.target_tool_index = self.tool_call_ids.get(id).copied();
        let result = update(self);
        self.target_tool_index = None;
        result
    }

    /// 【终端】【索引维护】移除响应文字后修正工具索引，避免后续事件指向相邻单元。
    /// 参数：删除的单元下标；返回无。
    pub(super) fn removed_cell(&mut self, removed: usize) {
        self.tool_call_ids.retain(|_, index| {
            if *index == removed {
                return false;
            }
            if *index > removed {
                *index -= 1;
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::tool_cell::ToolCell;

    /// 【终端】【并发回归】同名工具逆序结束时结果和进度仍对应各自参数。
    #[test]
    fn same_named_calls_update_by_id_in_completion_order() {
        let mut store = TranscriptStore::new(100);
        store.push_identified_tool_call("slow", "read_file".into(), "slow.rs".into());
        store.push_identified_tool_call("fast", "read_file".into(), "fast.rs".into());
        store.with_identified_tool("fast", "read_file", |store| {
            store.push_tool_progress("read_file".into(), "fast progress".into())
        });
        store.with_identified_tool("fast", "read_file", |store| {
            store.push_tool_result("read_file".into(), true, "fast result".into())
        });
        store.with_identified_tool("slow", "read_file", |store| {
            store.push_tool_result("read_file".into(), true, "slow result".into())
        });
        for (index, expected) in [(0, "slow"), (1, "fast")] {
            let HistoryCell::Tool(ToolCell::Invocation(view)) = &store.cells[index] else {
                panic!("tool expected");
            };
            assert_eq!(view.arguments, format!("{expected}.rs"));
            assert_eq!(
                view.outcome.as_ref().unwrap().output,
                format!("{expected} result")
            );
        }
        assert_eq!(store.cells.len(), 2);
        store.clear();
        assert!(store.tool_call_ids.is_empty());
    }
}

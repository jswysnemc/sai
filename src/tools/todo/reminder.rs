use super::store::TodoStore;
use anyhow::Result;
use std::path::PathBuf;

const REMINDER_AFTER_TOOL_ROUNDS: usize = 3;

/// 当前 Agent 轮次的 TODO 提醒计数器。
pub(crate) struct TodoReminder {
    store: TodoStore,
    unchanged_rounds: usize,
    injected: bool,
}

impl TodoReminder {
    /// 创建绑定当前会话文件的提醒计数器。
    ///
    /// 参数:
    /// - `file`: 当前会话 TODO 文件
    ///
    /// 返回:
    /// - 提醒计数器
    pub(crate) fn new(file: PathBuf) -> Self {
        Self {
            store: TodoStore::new(file),
            unchanged_rounds: 0,
            injected: false,
        }
    }

    /// 记录一个工具轮并按阈值生成提醒。
    ///
    /// 参数:
    /// - `updated`: 本轮是否成功修改 TODO 清单
    ///
    /// 返回:
    /// - 达到阈值时返回系统提醒文本
    pub(crate) fn after_tool_round(&mut self, updated: bool) -> Result<Option<String>> {
        if updated || !self.store.has_unfinished()? {
            self.unchanged_rounds = 0;
            return Ok(None);
        }
        self.unchanged_rounds += 1;
        if self.injected || self.unchanged_rounds < REMINDER_AFTER_TOOL_ROUNDS {
            return Ok(None);
        }
        self.injected = true;
        Ok(Some(format!(
            "<system-reminder>当前会话仍有未完成 TODO，且连续 {} 个工具轮没有更新清单。请使用 todo 工具核对并更新进度后继续。</system-reminder>",
            self.unchanged_rounds
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证未完成项连续三个工具轮未更新时才提醒。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn reminds_after_three_unchanged_tool_rounds() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("todos.json");
        TodoStore::new(file.clone()).add("unfinished").unwrap();
        let mut reminder = TodoReminder::new(file);

        assert!(reminder.after_tool_round(false).unwrap().is_none());
        assert!(reminder.after_tool_round(false).unwrap().is_none());
        assert!(reminder.after_tool_round(false).unwrap().is_some());
        assert!(reminder.after_tool_round(false).unwrap().is_none());
    }

    /// 验证 TODO 更新会重置连续轮次计数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn update_resets_unchanged_rounds() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("todos.json");
        TodoStore::new(file.clone()).add("unfinished").unwrap();
        let mut reminder = TodoReminder::new(file);

        assert!(reminder.after_tool_round(false).unwrap().is_none());
        assert!(reminder.after_tool_round(false).unwrap().is_none());
        assert!(reminder.after_tool_round(true).unwrap().is_none());
        assert!(reminder.after_tool_round(false).unwrap().is_none());
    }
}

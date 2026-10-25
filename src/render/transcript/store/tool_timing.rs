use super::super::cell::HistoryCell;
use super::super::tool_cell::ToolCell;
use super::TranscriptStore;

impl TranscriptStore {
    /// 【终端】【工具计时】新调用创建完成后启动展示计时，工具结果负责冻结。
    /// 参数: 无；返回: 无。
    pub(super) fn start_current_tool_timer(&mut self) {
        if let Some(HistoryCell::Tool(ToolCell::Invocation(view))) = self
            .active_tool_index
            .and_then(|index| self.cells.get_mut(index))
        {
            view.start_timer();
        }
    }

    /// 【终端】【历史耗时】恢复调用后清除刚创建的计时器，避免伪造历史耗时。
    /// 参数: 无；返回: 无。
    pub(super) fn clear_restored_tool_timer(&mut self) {
        if let Some(HistoryCell::Tool(ToolCell::Invocation(view))) = self
            .active_tool_index
            .and_then(|index| self.cells.get_mut(index))
        {
            view.clear_timer();
        }
    }
}

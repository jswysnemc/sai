use super::cell::HistoryCell;
use super::store::TranscriptStore;
use super::tool_cell::ToolCell;

impl TranscriptStore {
    /// 追加当前活动命令的实时输出。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `chunk`: 命令输出片段
    ///
    /// 返回:
    /// - 是否找到活动命令
    pub(crate) fn push_command_output(
        &mut self,
        name: &str,
        chunk: &crate::tools::command::CommandOutputChunk,
    ) -> bool {
        self.update_active_tool(name, |view| {
            view.append_command_output(chunk.stream, &chunk.bytes, chunk.omitted_bytes)
        })
    }

    /// 切换最近一个可折叠块的展开状态。
    ///
    /// 优先切换最近命令输出；若无则切换最近思考段落。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 找到可切换单元时返回 true
    pub(crate) fn toggle_latest_command_output(&mut self) -> bool {
        // 1. 从后往前找最近的命令输出
        for index in (0..self.cells.len()).rev() {
            let toggled = match self.cells.get_mut(index) {
                Some(HistoryCell::Tool(ToolCell::Invocation(view))) if view.has_command_output() => {
                    view.toggle_command_expanded();
                    true
                }
                _ => false,
            };
            if toggled {
                self.mark_dirty(index);
                return true;
            }
        }
        // 2. 否则切换最近思考段落
        for index in (0..self.cells.len()).rev() {
            if let Some(HistoryCell::Reasoning(cell)) = self.cells.get_mut(index) {
                cell.toggle_expanded();
                self.mark_dirty(index);
                return true;
            }
        }
        false
    }
}

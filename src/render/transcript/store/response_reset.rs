use super::*;

impl TranscriptStore {
    /// 【终端】【响应恢复】撤销失败尝试的正文、推理和未执行工具预览
    /// 参数：正文与推理字符数；返回无
    pub(crate) fn rewind_response(&mut self, mut content: usize, mut reasoning: usize) {
        self.finalize_live_tail();
        self.live_tool_call = None;
        let mut index = self.cells.len();
        while index > 0 && content + reasoning > 0 {
            index -= 1;
            let empty = match &mut self.cells[index] {
                HistoryCell::Markdown(cell) if content > 0 => {
                    content = crate::llm::remove_text_suffix(&mut cell.source, content);
                    cell.table_layouts = Default::default();
                    cell.source.is_empty()
                }
                HistoryCell::Reasoning(cell) if reasoning > 0 => {
                    reasoning = crate::llm::remove_text_suffix(&mut cell.source, reasoning);
                    cell.source.is_empty()
                }
                _ => false,
            };
            if empty {
                self.cells.remove(index);
                self.removed_cell(index);
            }
        }
        self.cache.clear();
        self.timeline_cache.clear();
        self.mark_dirty(index);
        self.refresh_active_tool_index();
    }
}

use super::TranscriptStore;
use crate::llm::ChatStreamKind;
use crate::render::transcript::store::HistoryCell;

impl TranscriptStore {
    /// 切换当前实时思考块的展开状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前没有可切换思考块时返回 false
    pub(crate) fn toggle_live_reasoning(&mut self) -> bool {
        let Some(tail) = self
            .live_tail
            .as_mut()
            .filter(|tail| tail.kind == ChatStreamKind::Reasoning && !tail.source.is_empty())
        else {
            return false;
        };
        tail.expanded = !tail.expanded;
        true
    }

    /// 切换最近一个 diff 块的展开状态。
    ///
    /// 只作用于最后一个：Ctrl+O 是"展开我刚看到的那块"，
    /// 一次性展开全部历史 diff 会把屏幕冲掉。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 没有可切换的 diff 块时返回 false
    pub(crate) fn toggle_last_diff(&mut self) -> bool {
        let Some(cell) = self.cells.iter_mut().rev().find_map(|cell| match cell {
            HistoryCell::Diff(cell) => Some(cell),
            _ => None,
        }) else {
            return false;
        };
        cell.toggle_expanded();
        true
    }
}

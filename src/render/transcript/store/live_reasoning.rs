use super::TranscriptStore;
use crate::llm::ChatStreamKind;

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
}

use super::TranscriptStore;
use crate::llm::ChatStreamKind;
#[cfg(test)]
use crate::render::transcript::store::HistoryCell;
#[cfg(test)]
use crate::render::transcript::tool_cell::ToolCell;

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

    /// 切换最近一块可折叠内容：live 思考、diff、定稿思考、命令输出或粘贴回显。
    ///
    /// Ctrl+O 只切「刚看到的那块」，并失效渲染缓存，否则定稿 diff 展开后仍画旧行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 找到可切换块时返回 true
    #[cfg(test)]
    pub(crate) fn toggle_inline_expand(&mut self) -> bool {
        if self.toggle_live_reasoning() {
            return true;
        }
        for index in (0..self.cells.len()).rev() {
            let toggled = match self.cells.get_mut(index) {
                Some(HistoryCell::Diff(cell)) => {
                    cell.toggle_expanded();
                    true
                }
                Some(HistoryCell::Reasoning(cell)) if !cell.source.trim().is_empty() => {
                    cell.toggle_expanded();
                    true
                }
                Some(HistoryCell::Tool(ToolCell::Invocation(view)))
                    if view.has_command_output() =>
                {
                    view.toggle_command_expanded();
                    true
                }
                Some(HistoryCell::UserEcho(cell))
                    if super::super::user_echo_cell::would_fold(cell) =>
                {
                    cell.toggle_expanded();
                    true
                }
                _ => false,
            };
            if toggled {
                self.mark_dirty(index);
                return true;
            }
        }
        false
    }
}

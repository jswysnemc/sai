use super::ReplInputDraft;
use crate::cli::repl_clipboard::ReplClipboardState;
use crate::paths::SaiPaths;
use crate::state::input_history::{self, InputHistoryEntry};

/// 【终端】【历史输入】从历史条目恢复可编辑草稿。
/// 参数: `entry` 为保存的输入
/// 返回: 正文与附件状态
pub(in crate::cli) fn restore_history_entry(entry: &InputHistoryEntry) -> ReplInputDraft {
    ReplInputDraft {
        text: entry.text.clone(),
        clipboard_state: ReplClipboardState::from_history_entry(entry),
    }
}

/// 【终端】【历史输入】同步维护内存和磁盘中的完整历史，保存失败不打断会话。
/// 参数: `paths` 为应用路径，`entries` 为内存历史，`entry` 为提交前冻结的快照
/// 返回: 无
pub(in crate::cli) fn remember_history(
    paths: &SaiPaths,
    entries: &mut Vec<InputHistoryEntry>,
    entry: InputHistoryEntry,
) {
    let _ = input_history::append_input_history_entry(paths, &entry);
    input_history::push_input_history_entry(entries, entry);
}

use crate::cli::repl_clipboard::ReplClipboardState;
use crate::state::input_history::InputHistoryEntry;

/// 【终端】【提交恢复】将提交前的输入快照交还输入框。
/// 参数: entry 为提交快照，text 和 clipboard 为下一次输入框的预填状态
/// 返回: 无
pub(super) fn restore_submitted_input(
    entry: &InputHistoryEntry,
    text: &mut Option<String>,
    clipboard: &mut Option<ReplClipboardState>,
) {
    let draft = crate::cli::repl_input::history::restore_history_entry(entry);
    *text = Some(draft.text);
    *clipboard = Some(draft.clipboard_state);
}

#[cfg(test)]
#[path = "input_restore_tests.rs"]
mod tests;

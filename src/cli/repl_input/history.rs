use super::ReplInputDraft;
use super::{
    chrome_input_content_cols, move_cursor_down_by_visual_row, move_cursor_up_by_visual_row,
    repl_history_is_clean, repl_should_browse_history, terminal_cols,
};
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

/// 【终端输入】【上移导航】优先在多行草稿内按视觉行上移，首行时按条件回溯输入历史。
/// 参数: `input`/`cursor`/`clipboard_state`/`history_index`/`history_clean_index`/`slash_selection`/`is_pasted` 为输入状态，`history` 为历史记录；返回: 是否需要重绘。
#[allow(clippy::too_many_arguments)]
pub(super) fn navigate_up(
    input: &mut String,
    cursor: &mut usize,
    clipboard_state: &mut ReplClipboardState,
    history: &[InputHistoryEntry],
    history_index: &mut usize,
    history_clean_index: &mut Option<usize>,
    slash_selection: &mut usize,
    is_pasted: &mut bool,
) -> bool {
    // 1. 【终端输入】【行内移动】折行宽度与输入框内容区一致，避免视觉落点偏移
    if let Some(next_cursor) =
        move_cursor_up_by_visual_row("", input, *cursor, chrome_input_content_cols(terminal_cols()))
    {
        *cursor = next_cursor;
        return true;
    }
    // 2. 【终端输入】【历史回溯】仅在空草稿或未改动的历史条目上向上翻页
    if repl_should_browse_history(input, history, *history_clean_index) {
        if input.is_empty() {
            *history_index = history.len();
        }
        *history_index = history_index.saturating_sub(1);
        let draft = restore_history_entry(&history[*history_index]);
        *input = draft.text;
        *clipboard_state = draft.clipboard_state;
        *cursor = input.chars().count();
        *history_clean_index = Some(*history_index);
        *slash_selection = 0;
        *is_pasted = false;
        return true;
    }
    false
}

/// 【终端输入】【下移导航】优先在多行草稿内按视觉行下移，末行时向后浏览历史或回到空白草稿。
/// 参数: `input`/`cursor`/`clipboard_state`/`history_index`/`history_clean_index`/`slash_selection`/`is_pasted` 为输入状态，`history` 为历史记录；返回: 无。
#[allow(clippy::too_many_arguments)]
pub(super) fn navigate_down(
    input: &mut String,
    cursor: &mut usize,
    clipboard_state: &mut ReplClipboardState,
    history: &[InputHistoryEntry],
    history_index: &mut usize,
    history_clean_index: &mut Option<usize>,
    slash_selection: &mut usize,
    is_pasted: &mut bool,
) {
    // 1. 【终端输入】【行内移动】多行输入尚未到底时移动视觉行光标
    if let Some(next_cursor) = move_cursor_down_by_visual_row(
        "",
        input,
        *cursor,
        chrome_input_content_cols(terminal_cols()),
    ) {
        *cursor = next_cursor;
        return;
    }
    // 2. 【终端输入】【历史前进】未修改历史时继续向新条目推进，越过末尾则清空草稿
    if repl_history_is_clean(input, history, *history_clean_index)
        && *history_index + 1 < history.len()
    {
        *history_index += 1;
        let draft = restore_history_entry(&history[*history_index]);
        *input = draft.text;
        *clipboard_state = draft.clipboard_state;
        *cursor = input.chars().count();
        *history_clean_index = Some(*history_index);
        *slash_selection = 0;
        *is_pasted = false;
    } else if repl_history_is_clean(input, history, *history_clean_index)
        && *history_index < history.len()
    {
        *history_index = history.len();
        input.clear();
        *cursor = 0;
        *history_clean_index = None;
        *slash_selection = 0;
        clipboard_state.clear();
        *is_pasted = false;
    }
}

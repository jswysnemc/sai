use super::*;
use crate::cli::repl_mentions::find_mention_trigger;
use crate::cli::repl_runtime::CompletionPanelKind;

/// 闲置输入循环中处理补全鼠标事件后的流程跳转结果。
pub(super) enum IdleCompletionStep {
    Ignored,
    Consumed,
    Redraw,
    Submit,
}

/// 【终端】【Tab 补全】依次尝试引用补全或斜杠命令补全，更新输入与光标。
pub(super) fn apply_tab_completion(
    runtime: &ReplRuntime,
    input: &mut String,
    cursor: &mut usize,
    slash_selection: &mut usize,
    clipboard: &mut ReplClipboardState,
    history_clean_index: &mut Option<usize>,
) {
    if let Some((next, next_cursor)) =
        complete_active_mention(input, *cursor, *slash_selection, runtime, clipboard)
    {
        *input = next;
        *cursor = next_cursor;
        *slash_selection = 0;
        *history_clean_index = None;
    } else if input.starts_with('/') {
        if let Some(completed) =
            crate::cli::repl_completion::accept(input, *cursor, *slash_selection, false, false)
        {
            *input = completed.to_string();
            *cursor = input.chars().count();
            *history_clean_index = None;
        }
        *slash_selection = 0;
    }
}

/// 【终端】【闲置补全鼠标】处理悬停、滚轮和左键点选，避免闲置输入主循环膨胀。
#[allow(clippy::too_many_arguments)]
pub(super) fn process_idle_completion_mouse(
    runtime: &mut ReplRuntime,
    event: &Event,
    input: &mut String,
    cursor: &mut usize,
    slash_selection: &mut usize,
    clipboard: &mut ReplClipboardState,
    history_clean_index: &mut Option<usize>,
    is_pasted: &mut bool,
) -> IdleCompletionStep {
    match runtime.handle_completion_mouse(event, input, *cursor, *slash_selection) {
        crate::cli::repl_runtime::CompletionMouseEffect::Ignored => IdleCompletionStep::Ignored,
        crate::cli::repl_runtime::CompletionMouseEffect::Consumed => IdleCompletionStep::Consumed,
        crate::cli::repl_runtime::CompletionMouseEffect::Redraw => IdleCompletionStep::Redraw,
        crate::cli::repl_runtime::CompletionMouseEffect::Select(next) => {
            *slash_selection = next;
            IdleCompletionStep::Redraw
        }
        crate::cli::repl_runtime::CompletionMouseEffect::Accept(kind, index, disabled) => {
            if !disabled
                && apply_completion_click(
                    runtime,
                    input,
                    cursor,
                    slash_selection,
                    clipboard,
                    kind,
                    index,
                )
            {
                return IdleCompletionStep::Submit;
            }
            *history_clean_index = None;
            *is_pasted = false;
            IdleCompletionStep::Redraw
        }
    }
}

/// 【终端】【补全点选】将鼠标点击的候选命令或引用写入草稿；完整命令已就绪时返回 true 以便直接提交。
///
/// 参数:
/// - `runtime`: REPL 运行期
/// - `input`: 输入草稿文本
/// - `cursor`: 光标字符偏移
/// - `slash_selection`: 当前高亮候选序号
/// - `clipboard`: 剪贴板附件状态
/// - `kind`: 命中的补全面板类型
/// - `index`: 命中的候选下标
///
/// 返回:
/// - 已完成补全等待继续编辑返回 false；已是一条就绪命令返回 true
pub(super) fn apply_completion_click(
    runtime: &ReplRuntime,
    input: &mut String,
    cursor: &mut usize,
    slash_selection: &mut usize,
    clipboard: &mut ReplClipboardState,
    kind: CompletionPanelKind,
    index: usize,
) -> bool {
    *slash_selection = index;
    match kind {
        CompletionPanelKind::Mention => {
            if let Some((next, next_cursor)) =
                complete_active_mention(input, *cursor, index, runtime, clipboard)
            {
                *input = next;
                *cursor = next_cursor;
                *slash_selection = 0;
            }
            false
        }
        CompletionPanelKind::Slash => {
            if let Some(completed) =
                crate::cli::repl_completion::accept(input, *cursor, index, false, true)
            {
                *input = completed;
                *cursor = input.chars().count();
                *slash_selection = 0;
                false
            } else {
                true
            }
        }
    }
}

/// 确认当前引用建议，替换触发片段。
///
/// 参数:
/// - `input`: 当前输入
/// - `cursor`: 光标字符偏移
/// - `clipboard`: 接收原子块登记的输入状态
/// - `selected`: 选中下标
/// - `runtime`: 异步候选运行期
///
/// 返回:
/// - 新输入与新光标；无建议时为空
pub(super) fn complete_active_mention(
    input: &str,
    cursor: usize,
    selected: usize,
    runtime: &ReplRuntime,
    clipboard: &mut ReplClipboardState,
) -> Option<(String, usize)> {
    let trigger = find_mention_trigger(input, cursor)?;
    let suggestions = runtime.mention_candidates(input, cursor);
    if runtime.mention_completion_pending() {
        return Some((input.to_string(), cursor));
    }
    let item = suggestions.get(selected.min(suggestions.len().saturating_sub(1)))?;
    Some(clipboard.complete_mention(input, &trigger, item))
}

/// 判断当前输入是否仍与选中的历史记录一致。
///
/// 参数:
/// - `input`: 当前输入
/// - `history`: 历史记录
/// - `history_clean_index`: 最近选中的历史下标
///
/// 返回:
/// - 未修改选中历史时返回 true
pub(in crate::cli) fn repl_history_is_clean(
    input: &str,
    history: &[impl AsRef<str>],
    history_clean_index: Option<usize>,
) -> bool {
    history_clean_index
        .and_then(|index| history.get(index))
        .is_some_and(|entry| entry.as_ref() == input)
}

/// 判断上方向键是否可以进入历史浏览。
///
/// 参数:
/// - `input`: 当前输入
/// - `history`: 历史记录
/// - `history_clean_index`: 最近选中的历史下标
///
/// 返回:
/// - 输入为空或仍为未修改历史时返回 true
pub(in crate::cli) fn repl_should_browse_history(
    input: &str,
    history: &[impl AsRef<str>],
    history_clean_index: Option<usize>,
) -> bool {
    !history.is_empty()
        && (input.is_empty() || repl_history_is_clean(input, history, history_clean_index))
}

/// 循环切换 REPL 权限模式。
///
/// 参数:
/// - `mode`: 当前模式
///
/// 返回:
/// - 下一模式
pub(super) fn cycle_repl_mode(mode: AgentMode) -> AgentMode {
    mode.next()
}

/// 【终端输入】【原子删除】参数 clipboard/input/cursor 为编辑状态，backward 为向前删除；返回内容是否变化
pub(super) fn delete_input_atom(
    clipboard: &mut ReplClipboardState,
    input: &mut String,
    cursor: &mut usize,
    backward: bool,
) -> bool {
    if backward {
        if clipboard.remove_block_before_cursor(input, cursor) {
            return true;
        }
        if *cursor == 0 {
            return false;
        }
        remove_char_before_cursor(input, cursor);
    } else {
        if clipboard.remove_block_at_cursor(input, *cursor) {
            return true;
        }
        if *cursor >= input.chars().count() {
            return false;
        }
        remove_char_at_cursor(input, *cursor);
    }
    true
}

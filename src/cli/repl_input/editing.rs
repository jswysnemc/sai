use super::*;
use crate::cli::repl_mentions::{find_mention_trigger, mention_suggestions, MentionSuggestion};

/// 返回光标处可见的引用建议。
///
/// 参数:
/// - `input`: 当前输入
/// - `cursor`: 光标字符偏移
/// - `skills`: skill 目录
///
/// 返回:
/// - 过滤后的建议
pub(super) fn active_mention_suggestions(
    input: &str,
    cursor: usize,
    skills: &[(String, String)],
) -> Vec<MentionSuggestion> {
    find_mention_trigger(input, cursor)
        .map(|trigger| mention_suggestions(&trigger, skills))
        .unwrap_or_default()
}

/// 确认当前引用建议，替换触发片段。
///
/// 参数:
/// - `input`: 当前输入
/// - `cursor`: 光标字符偏移
/// - `clipboard`: 接收原子块登记的输入状态
/// - `selected`: 选中下标
/// - `skills`: skill 目录
///
/// 返回:
/// - 新输入与新光标；无建议时为空
pub(super) fn complete_active_mention(
    input: &str,
    cursor: usize,
    selected: usize,
    skills: &[(String, String)],
    clipboard: &mut ReplClipboardState,
) -> Option<(String, usize)> {
    let trigger = find_mention_trigger(input, cursor)?;
    let suggestions = mention_suggestions(&trigger, skills);
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
    history: &[String],
    history_clean_index: Option<usize>,
) -> bool {
    history_clean_index
        .and_then(|index| history.get(index))
        .is_some_and(|entry| entry == input)
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
    history: &[String],
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
    match mode {
        AgentMode::Yolo => AgentMode::Audited,
        AgentMode::Audited => AgentMode::AutoAudit,
        AgentMode::AutoAudit => AgentMode::Plan,
        AgentMode::Plan => AgentMode::Yolo,
    }
}

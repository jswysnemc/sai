use super::repository::load_memory;
use crate::state::ConversationDb;
use anyhow::Result;

const MAX_MEMORY_CONTEXT_CHARS: usize = 8_000;
const MIN_HISTORY_CHARS: usize = 1_000;
const PREFIX: &str = "<session-memory>\nSupplementary session memory: retain applicable facts, constraints and decisions in the summary. The source conversation and newer explicit user requests take precedence over this memory.\n";
const SUFFIX: &str = "\n</session-memory>\n\n";

/// 【会话记忆】【压缩输入】在保留历史预算的前提下提供会话记忆补充材料。
/// 参数：数据库、会话标识、窗口容量和已预留字符数；返回：带边界的记忆文本，无空间时为空。
pub(crate) fn render_compaction_memory(
    db: &ConversationDb,
    session_id: &str,
    context_limit_chars: usize,
    reserved_chars: usize,
) -> Result<String> {
    let available = context_limit_chars
        .saturating_sub(reserved_chars)
        .saturating_sub(MIN_HISTORY_CHARS);
    let budget = available
        .min(context_limit_chars / 10)
        .min(MAX_MEMORY_CONTEXT_CHARS);
    let body_budget = budget.saturating_sub(PREFIX.chars().count() + SUFFIX.chars().count());
    if body_budget == 0 {
        return Ok(String::new());
    }
    let Some(memory) = load_memory(db, session_id)? else {
        return Ok(String::new());
    };
    let body = memory
        .summary
        .trim()
        .chars()
        .take(body_budget)
        .collect::<String>();
    if body.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("{PREFIX}{body}{SUFFIX}"))
}

#[cfg(test)]
#[path = "compaction_context_tests.rs"]
mod tests;

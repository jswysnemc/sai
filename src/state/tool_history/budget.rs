use super::model::{ToolCallStatus, ToolExchangeRecord};
use super::repository::load_tool_exchanges_for_turn;
use crate::state::tool_history::format_legacy_tool_reports;
use crate::state::turns::{Turn, TurnStatus};
use crate::state::ConversationDb;
use anyhow::Result;
use std::path::{Path, PathBuf};

const TOOL_ARGUMENT_MAX_CHARS: usize = 1_000;
const TOOL_RESULT_MAX_CHARS: usize = 2_000;
const TOOL_TURN_MAX_CHARS: usize = 6_000;
const MIN_TOTAL_HISTORY_BUDGET_CHARS: usize = 1_000;

/// 压缩摘要历史预算结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::state) struct BudgetedSummaryHistory {
    pub history: String,
    pub chars: usize,
    pub replacement_missing_count: usize,
    pub result_ref_missing_file_count: usize,
    pub clipped_result_count: usize,
    pub clipped_turn_count: usize,
    pub clipped_total_history: bool,
}

/// 构造带工具历史预算的压缩摘要历史文本。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `state_dir`: 可选会话状态目录，用于校验 result_ref 文件
/// - `turns`: 需要压缩的轮次
/// - `total_budget_chars`: 历史文本总预算
///
/// 返回:
/// - 已完成裁剪和统计的历史文本
pub(in crate::state) fn build_budgeted_summary_history(
    db: &ConversationDb,
    session_id: &str,
    state_dir: Option<&Path>,
    turns: &[Turn],
    total_budget_chars: usize,
) -> Result<BudgetedSummaryHistory> {
    let mut result = BudgetedSummaryHistory::default();
    if total_budget_chars < MIN_TOTAL_HISTORY_BUDGET_CHARS {
        result.clipped_total_history = !turns.is_empty();
        return Ok(result);
    }
    let mut remaining = total_budget_chars;
    let mut parts = Vec::new();
    for turn in turns {
        let exchanges = load_tool_exchanges_for_turn(db, session_id, &turn.turn_id)?;
        let (mut text, stats) = format_turn_for_summary(turn, state_dir, &exchanges);
        result.replacement_missing_count += stats.replacement_missing_count;
        result.result_ref_missing_file_count += stats.result_ref_missing_file_count;
        result.clipped_result_count += stats.clipped_result_count;
        if text.chars().count() > TOOL_TURN_MAX_CHARS {
            text = truncate_chars(&text, TOOL_TURN_MAX_CHARS);
            result.clipped_turn_count += 1;
        }
        let text_chars = text.chars().count();
        let separator_chars = usize::from(!parts.is_empty()) * 2;
        if text_chars + separator_chars > remaining {
            let allowed = remaining.saturating_sub(separator_chars);
            if allowed > 0 {
                parts.push(truncate_chars(&text, allowed));
            }
            result.clipped_total_history = true;
            break;
        }
        remaining = remaining.saturating_sub(text_chars + separator_chars);
        parts.push(text);
    }
    result.history = parts.join("\n\n");
    result.chars = result.history.chars().count();
    Ok(result)
}

/// 格式化单个轮次为摘要输入。
///
/// 参数:
/// - `turn`: 对话轮次
/// - `state_dir`: 可选会话状态目录
/// - `exchanges`: 该轮结构化工具调用记录
///
/// 返回:
/// - 轮次文本和裁剪统计
fn format_turn_for_summary(
    turn: &Turn,
    state_dir: Option<&Path>,
    exchanges: &[ToolExchangeRecord],
) -> (String, BudgetedSummaryHistory) {
    let mut stats = BudgetedSummaryHistory::default();
    let mut parts = vec![
        format!(
            "<turn id=\"{}\" status=\"{}\">",
            escape_attr(&turn.turn_id),
            status_name(turn.status)
        ),
        format!("<user>\n{}\n</user>", turn.user_content.trim()),
    ];
    if let Some(reasoning) = turn
        .assistant_reasoning
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!(
            "<assistant-reasoning>\n{reasoning}\n</assistant-reasoning>"
        ));
    }
    if !exchanges.is_empty() {
        let mut tool_parts = Vec::new();
        for exchange in exchanges {
            let (tool_text, tool_stats) = format_tool_exchange(exchange, state_dir);
            stats.replacement_missing_count += tool_stats.replacement_missing_count;
            stats.result_ref_missing_file_count += tool_stats.result_ref_missing_file_count;
            stats.clipped_result_count += tool_stats.clipped_result_count;
            tool_parts.push(tool_text);
        }
        parts.push(format!(
            "<tool-history>\n{}\n</tool-history>",
            tool_parts.join("\n\n")
        ));
    }
    parts.push(format!(
        "<assistant>\n{}\n</assistant>",
        turn.assistant_content.trim()
    ));
    if exchanges.is_empty() && !turn.tool_reports.is_empty() {
        parts.push(format!(
            "<legacy-tool-reports>\n{}\n</legacy-tool-reports>",
            format_legacy_tool_reports(&turn.tool_reports, TOOL_RESULT_MAX_CHARS)
        ));
    }
    parts.push("</turn>".to_string());
    (parts.join("\n"), stats)
}

/// 格式化单个工具交换记录。
///
/// 参数:
/// - `exchange`: 工具调用和结果记录
/// - `state_dir`: 可选会话状态目录
///
/// 返回:
/// - 工具历史文本和裁剪统计
fn format_tool_exchange(
    exchange: &ToolExchangeRecord,
    state_dir: Option<&Path>,
) -> (String, BudgetedSummaryHistory) {
    let mut stats = BudgetedSummaryHistory::default();
    let (result_text, ok, result_ref, original_chars) = match &exchange.result {
        Some(result) => {
            let replacement_missing = result.result_ref.is_some() && exchange.replacement.is_none();
            if replacement_missing {
                stats.replacement_missing_count += 1;
            }
            if result_ref_file_missing(state_dir, result.result_ref.as_deref()) {
                stats.result_ref_missing_file_count += 1;
            }
            let visible = exchange
                .replacement
                .as_ref()
                .map(|replacement| replacement.replacement.as_str())
                .unwrap_or(result.result_preview.as_str());
            (
                visible.to_string(),
                Some(result.ok),
                result.result_ref.as_deref(),
                Some(result.original_chars),
            )
        }
        None if exchange.call.status == ToolCallStatus::Interrupted => (
            "tool error: tool call was interrupted before a result was recorded".to_string(),
            Some(false),
            None,
            None,
        ),
        None => (
            "tool error: tool result is missing from durable history".to_string(),
            Some(false),
            None,
            None,
        ),
    };
    let clipped_result = truncate_chars(result_text.trim(), TOOL_RESULT_MAX_CHARS);
    if clipped_result.chars().count() < result_text.trim().chars().count() {
        stats.clipped_result_count += 1;
    }
    let arguments = truncate_chars(exchange.call.arguments.trim(), TOOL_ARGUMENT_MAX_CHARS);
    let result_ref_attr = result_ref
        .map(|value| format!(" result_ref=\"{}\"", escape_attr(value)))
        .unwrap_or_default();
    let original_chars_attr = original_chars
        .map(|value| format!(" original_chars=\"{value}\""))
        .unwrap_or_default();
    let ok_attr = ok
        .map(|value| format!(" ok=\"{}\"", if value { "true" } else { "false" }))
        .unwrap_or_default();
    (
        format!(
            "<tool-call id=\"{}\" name=\"{}\" status=\"{}\">\n<arguments>\n{}\n</arguments>\n<result{}{}{}>\n{}\n</result>\n</tool-call>",
            escape_attr(&exchange.call.provider_call_id),
            escape_attr(&exchange.call.tool_name),
            exchange.call.status.as_str(),
            arguments,
            ok_attr,
            result_ref_attr,
            original_chars_attr,
            clipped_result,
        ),
        stats,
    )
}

/// 判断 result_ref 文件是否缺失。
///
/// 参数:
/// - `state_dir`: 可选会话状态目录
/// - `result_ref`: 工具完整输出引用
///
/// 返回:
/// - 是否存在引用但文件缺失
fn result_ref_file_missing(state_dir: Option<&Path>, result_ref: Option<&str>) -> bool {
    let Some(state_dir) = state_dir else {
        return false;
    };
    let Some(result_ref) = result_ref else {
        return false;
    };
    let path = PathBuf::from(result_ref);
    let full_path = if path.is_absolute() {
        path
    } else {
        state_dir.join(path)
    };
    !full_path.is_file()
}

/// 按字符数截断文本。
///
/// 参数:
/// - `value`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 截断后的文本
fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut iter = value.chars();
    let truncated = iter.by_ref().take(max_chars).collect::<String>();
    if iter.next().is_some() {
        format!("{truncated}\n[truncated]")
    } else {
        truncated
    }
}

/// 转义 XML 属性文本。
///
/// 参数:
/// - `value`: 原始属性值
///
/// 返回:
/// - 转义后的属性值
fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 返回轮次状态名称。
///
/// 参数:
/// - `status`: 轮次状态
///
/// 返回:
/// - 状态名称
fn status_name(status: TurnStatus) -> &'static str {
    match status {
        TurnStatus::Running => "running",
        TurnStatus::Completed => "completed",
        TurnStatus::Interrupted => "interrupted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tool_history::repository::{insert_tool_call, insert_tool_result};
    use crate::state::tool_history::schema::create_tool_history_tables;
    use crate::state::tool_history::{NewToolCallRecord, NewToolResultRecord};

    fn test_db() -> (tempfile::TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        let conn = db.conn.lock().unwrap();
        create_tool_history_tables(&conn).unwrap();
        drop(conn);
        (temp, db)
    }

    fn turn(id: &str) -> Turn {
        Turn {
            turn_id: id.to_string(),
            seq: 1,
            user_content: "inspect".to_string(),
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: "done".to_string(),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
        }
    }

    #[test]
    fn summary_history_uses_tool_result_preview_and_budget() {
        let (_temp, db) = test_db();
        insert_tool_call(
            &db,
            NewToolCallRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                seq: 1,
                provider_call_id: "call_1".to_string(),
                tool_name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        )
        .unwrap();
        insert_tool_result(
            &db,
            NewToolResultRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                provider_call_id: "call_1".to_string(),
                ok: true,
                result_preview: "tool preview".to_string(),
                result_ref: Some("tool-results/call_1.txt".to_string()),
                error: None,
                original_chars: 12_000,
            },
        )
        .unwrap();

        let history =
            build_budgeted_summary_history(&db, "default", None, &[turn("turn_1")], 10_000)
                .unwrap();

        assert!(history.history.contains("<tool-history>"));
        assert!(history.history.contains("tool preview"));
        assert_eq!(history.replacement_missing_count, 1);
    }

    #[test]
    fn summary_history_counts_missing_result_ref_file() {
        let (temp, db) = test_db();
        insert_tool_call(
            &db,
            NewToolCallRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                seq: 1,
                provider_call_id: "call_1".to_string(),
                tool_name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        )
        .unwrap();
        insert_tool_result(
            &db,
            NewToolResultRecord {
                session_id: "default".to_string(),
                turn_id: "turn_1".to_string(),
                provider_call_id: "call_1".to_string(),
                ok: true,
                result_preview: "tool preview".to_string(),
                result_ref: Some("tool-results/missing.txt".to_string()),
                error: None,
                original_chars: 12_000,
            },
        )
        .unwrap();

        let history = build_budgeted_summary_history(
            &db,
            "default",
            Some(temp.path()),
            &[turn("turn_1")],
            10_000,
        )
        .unwrap();

        assert_eq!(history.result_ref_missing_file_count, 1);
    }

    #[test]
    fn total_history_budget_clips_old_context() {
        let (_temp, db) = test_db();
        let mut turns = vec![turn("turn_1"), turn("turn_2"), turn("turn_3")];
        for turn in &mut turns {
            turn.user_content = "inspect ".repeat(200);
            turn.assistant_content = "done ".repeat(200);
        }

        let history = build_budgeted_summary_history(&db, "default", None, &turns, 1_200).unwrap();

        assert!(history.chars <= 1_212);
        assert!(history.clipped_total_history);
    }
}

use super::model::{CompactionRequest, RunningTurnCompaction};
use super::should_compact_for_context_tokens;
use crate::state::turns::{Turn, TurnStatus};

/// 压缩后保留的最近已完成轮次数量。
///
/// 方案 B 下压缩会移除全部 assistant/tool 消息，用户消息由预算池单独保留，
/// 因此这里不再需要为"保留最近对话"预留轮次；保留 0 表示已完成轮次全部参与摘要。
pub const PRESERVED_RECENT_TURNS: usize = 0;

/// 运行中轮次末尾保留不压缩的工具调用条数。
///
/// 模型刚拿到的几条结果通常正是它下一步要用的，全部折进摘要会让它立刻重新调用。
pub const PRESERVED_RUNNING_TOOL_CALLS: usize = 4;

/// 使用统一策略选择需要压缩的会话内容。
///
/// 与旧策略的关键差异：运行中轮次不再被排除。单个用户问题触发数十次工具调用时，
/// 上下文膨胀发生在轮次内部，只压缩已完成轮次等于没有压缩。
///
/// 参数:
/// - `turns`: 当前全部轮次
/// - `previous_summary`: 已有压缩摘要
/// - `running_turn_call_count`: 运行中轮次已记录的工具调用条数
/// - `current_context_tokens`: 当前请求上下文估算
/// - `context_limit_tokens`: 当前模型上下文预算
/// - `force`: 是否由手动入口强制触发
///
/// 返回:
/// - 需要执行的压缩请求；没有任何可压缩内容时返回空
pub fn select_compaction(
    turns: &[Turn],
    previous_summary: Option<String>,
    running_turn_call_count: usize,
    current_context_tokens: usize,
    context_limit_tokens: usize,
    force: bool,
) -> Option<CompactionRequest> {
    if !force && !should_compact_for_context_tokens(current_context_tokens, context_limit_tokens) {
        return None;
    }
    // 1. 已完成轮次全部参与摘要，用户消息由保留预算单独兜住
    let completed = turns
        .iter()
        .filter(|turn| turn.status != TurnStatus::Running)
        .cloned()
        .collect::<Vec<_>>();
    let selected_len = completed.len().saturating_sub(PRESERVED_RECENT_TURNS);
    let compact_turns = completed.into_iter().take(selected_len).collect::<Vec<_>>();
    // 2. 运行中轮次同样参与：末尾若干条最新结果保留，其余折进摘要
    let running_turn = select_running_turn_range(turns, running_turn_call_count);
    let request =
        CompactionRequest::new(compact_turns, previous_summary).with_running_turn(running_turn);
    request.has_content().then_some(request)
}

/// 选择运行中轮次内应被摘要覆盖的工具调用范围。
///
/// 参数:
/// - `turns`: 当前全部轮次
/// - `running_turn_call_count`: 运行中轮次已记录的工具调用条数
///
/// 返回:
/// - 运行中轮次压缩范围；无运行轮次或调用数不足时返回空
fn select_running_turn_range(
    turns: &[Turn],
    running_turn_call_count: usize,
) -> Option<RunningTurnCompaction> {
    let running = turns
        .iter()
        .find(|turn| turn.status == TurnStatus::Running)?;
    let compacted_calls = running_turn_call_count.saturating_sub(PRESERVED_RUNNING_TOOL_CALLS);
    (compacted_calls > 0).then(|| RunningTurnCompaction {
        turn_id: running.turn_id.clone(),
        compacted_calls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造指定正文体积的已完成测试轮次。
    ///
    /// 参数:
    /// - `seq`: 轮次序号
    /// - `chars`: 用户与助手正文字符数
    ///
    /// 返回:
    /// - 已完成轮次
    fn completed_turn(seq: i64, chars: usize) -> Turn {
        Turn {
            turn_id: format!("turn_{seq}"),
            seq,
            user_content: "u".repeat(chars),
            user_image_urls: Vec::new(),
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: "a".repeat(chars),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
            duration_ms: 0,
            parent_turn_id: None,
            model: None,
            error: None,
        }
    }

    /// 构造运行中轮次。
    ///
    /// 参数:
    /// - `seq`: 轮次序号
    ///
    /// 返回:
    /// - 运行中轮次
    fn running_turn(seq: i64) -> Turn {
        let mut turn = completed_turn(seq, 100);
        turn.status = TurnStatus::Running;
        turn
    }

    /// 验证自动压缩在阈值处触发，已完成轮次全部参与。
    #[test]
    fn automatic_compaction_covers_all_completed_turns() {
        let turns = (1..=6)
            .map(|seq| completed_turn(seq, 100))
            .collect::<Vec<_>>();

        assert!(select_compaction(&turns, None, 0, 899, 1_000, false).is_none());
        let request = select_compaction(&turns, None, 0, 900, 1_000, false)
            .expect("automatic compaction request");

        assert_eq!(request.compact_turn_ids.len(), 6);
    }

    /// 验证运行中轮次的工具调用参与压缩，末尾若干条保留。
    #[test]
    fn running_turn_tool_calls_participate_in_compaction() {
        let turns = vec![completed_turn(1, 100), running_turn(2)];

        let request = select_compaction(&turns, None, 30, 900, 1_000, false)
            .expect("running turn must be compactable");

        let running = request.running_turn.expect("running turn range");
        assert_eq!(running.turn_id, "turn_2");
        assert_eq!(running.compacted_calls, 30 - PRESERVED_RUNNING_TOOL_CALLS);
    }

    /// 验证只有运行中轮次时压缩依然成立。
    ///
    /// 这是旧策略完全失效的场景：单个用户问题触发大量工具调用，
    /// 历史里没有足够的已完成轮次可供压缩。
    #[test]
    fn compacts_when_only_a_running_turn_exists() {
        let turns = vec![running_turn(1)];

        let request = select_compaction(&turns, None, 40, 900, 1_000, false)
            .expect("single running turn must still compact");

        assert!(request.compact_turns.is_empty());
        assert_eq!(
            request.running_turn.expect("running range").compacted_calls,
            40 - PRESERVED_RUNNING_TOOL_CALLS
        );
    }

    /// 验证运行中工具调用不足保留数量时不压缩轮次内容。
    #[test]
    fn skips_running_turn_below_preserved_calls() {
        let turns = vec![running_turn(1)];

        assert!(select_compaction(
            &turns,
            None,
            PRESERVED_RUNNING_TOOL_CALLS,
            900,
            1_000,
            false
        )
        .is_none());
    }

    /// 验证手动压缩绕过占用阈值。
    #[test]
    fn manual_compaction_bypasses_only_the_threshold() {
        let turns = (1..=5)
            .map(|seq| completed_turn(seq, 100))
            .collect::<Vec<_>>();

        let request = select_compaction(&turns, Some("summary".to_string()), 0, 100, 1_000, true)
            .expect("manual compaction request");

        assert_eq!(request.compact_turn_ids.len(), 5);
        assert_eq!(request.previous_summary.as_deref(), Some("summary"));
    }

    /// 验证没有任何可压缩内容时不创建空请求。
    #[test]
    fn skips_when_nothing_can_be_compacted() {
        assert!(select_compaction(&[], None, 0, 900, 1_000, false).is_none());
        assert!(select_compaction(&[], None, 0, 100, 1_000, true).is_none());
    }
}

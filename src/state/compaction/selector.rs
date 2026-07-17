use super::model::CompactionRequest;
use super::should_compact_for_context_tokens;
use crate::state::turns::{Turn, TurnStatus};

/// 压缩后保留的最近已完成轮次数量。
pub const PRESERVED_RECENT_TURNS: usize = 2;

/// 使用统一策略选择需要压缩的旧会话轮次。
///
/// 自动触发固定使用九成上下文阈值；手动触发只绕过阈值判断，
/// 两种入口都压缩最近两轮以前的全部非运行轮次。
///
/// 参数:
/// - `turns`: 当前全部轮次
/// - `previous_summary`: 已有压缩摘要
/// - `current_context_tokens`: 当前请求上下文 token 估算
/// - `context_limit_tokens`: 当前模型上下文窗口 token 数
/// - `force`: 是否由手动入口强制触发
///
/// 返回:
/// - 需要执行的压缩请求；没有足够旧轮次时返回空
pub fn select_compaction(
    turns: &[Turn],
    previous_summary: Option<String>,
    current_context_tokens: usize,
    context_limit_tokens: usize,
    force: bool,
) -> Option<CompactionRequest> {
    if !force && !should_compact_for_context_tokens(current_context_tokens, context_limit_tokens) {
        return None;
    }
    let non_running = turns
        .iter()
        .filter(|turn| turn.status != TurnStatus::Running)
        .cloned()
        .collect::<Vec<_>>();
    let selected_len = non_running.len().saturating_sub(PRESERVED_RECENT_TURNS);
    if selected_len == 0 {
        return None;
    }
    Some(CompactionRequest::new(
        non_running.into_iter().take(selected_len).collect(),
        previous_summary,
    ))
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
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: "a".repeat(chars),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
        }
    }

    /// 验证自动压缩只在固定九成阈值处触发，并统一保留最近两轮。
    #[test]
    fn automatic_compaction_uses_fixed_threshold_and_tail() {
        let turns = (1..=6)
            .map(|seq| completed_turn(seq, 100))
            .collect::<Vec<_>>();

        assert!(select_compaction(&turns, None, 899, 1_000, false).is_none());
        let request = select_compaction(&turns, None, 900, 1_000, false)
            .expect("automatic compaction request");

        assert_eq!(
            request.compact_turn_ids,
            vec![
                "turn_1".to_string(),
                "turn_2".to_string(),
                "turn_3".to_string(),
                "turn_4".to_string(),
            ]
        );
    }

    /// 验证手动压缩绕过占用阈值，但仍使用与自动压缩相同的轮次范围。
    #[test]
    fn manual_compaction_bypasses_only_the_threshold() {
        let turns = (1..=5)
            .map(|seq| completed_turn(seq, 100))
            .collect::<Vec<_>>();

        let request = select_compaction(&turns, Some("summary".to_string()), 100, 1_000, true)
            .expect("manual compaction request");

        assert_eq!(
            request.compact_turn_ids,
            vec![
                "turn_1".to_string(),
                "turn_2".to_string(),
                "turn_3".to_string(),
            ]
        );
        assert_eq!(request.previous_summary.as_deref(), Some("summary"));
    }

    /// 验证运行中轮次不参与选择，最近两个已结束轮次继续保留。
    #[test]
    fn excludes_running_turn_and_preserves_completed_tail() {
        let mut turns = (1..=4)
            .map(|seq| completed_turn(seq, 100))
            .collect::<Vec<_>>();
        let mut running = completed_turn(5, 100);
        running.status = TurnStatus::Running;
        turns.push(running);

        let request =
            select_compaction(&turns, None, 900, 1_000, false).expect("compaction request");

        assert_eq!(
            request.compact_turn_ids,
            vec!["turn_1".to_string(), "turn_2".to_string()]
        );
    }

    /// 验证仅有保留尾部时不创建无效压缩请求。
    #[test]
    fn skips_when_only_preserved_tail_exists() {
        let turns = vec![completed_turn(1, 100), completed_turn(2, 100)];

        assert!(select_compaction(&turns, None, 900, 1_000, false).is_none());
        assert!(select_compaction(&turns, None, 100, 1_000, true).is_none());
    }
}

use super::session_summary::render_session_summary;
use crate::llm::Usage;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::content_indent::align_to_guide_column;
use crate::state::{SessionSnapshot, ToolHistorySummary, UsageSnapshot};

#[test]
fn renders_compact_session_summary_with_key_fields() {
    let snapshot = SessionSnapshot {
        session_id: "default".to_string(),
        turn_count: 2,
        context_chars: 12_300,
        context_limit_chars: 128_000,
        context_ratio: 12_300.0 / 128_000.0,
        context_prompt_tokens: 8_000,
        context_window_tokens: 1_000_000,
        context_token_ratio: 8_000.0 / 1_000_000.0,
        checkpoint_count: 0,
        checkpoint_covered_turns: 0,
        tail_turns: 2,
        latest_checkpoint_at: None,
        latest_checkpoint_reason: None,
        usage: UsageSnapshot {
            requests: 1,
            prompt_tokens: 8_000,
            completion_tokens: 4_000,
            total_tokens: 12_000,
            last_usage: None,
            last_conversation_usage: Some(Usage {
                prompt_tokens: 8_000,
                completion_tokens: 4_000,
                total_tokens: 12_000,
                cache_read_tokens: 6_000,
                cache_write_tokens: 0,
            }),
        },
        compaction: None,
        recovery: crate::state::RecoverySnapshot::default(),
        context_epoch: None,
        session_memory: None,
        tool_history: ToolHistorySummary::default(),
        runtime_recovery: crate::runtime_recovery::RuntimeRecoverySummary::default(),
        dynamic_sources: Vec::new(),
        projection_warnings: Vec::new(),
        active_run: None,
        last_turn_duration_ms: 12_500,
    };

    let output = render_session_summary(&snapshot);

    // 行首是与助手正文同款的引导点，而非另一套 ▸ 符号
    assert!(!output.contains('▸'));
    assert!(output.starts_with("\x1b[2m•\x1b[0m "));
    assert!(output.contains("Context") || output.contains("上下文"));
    assert!(output.contains("8.0k"));
    assert!(output.contains("1000k"));
    assert!(output.contains("0.8%"));
    assert!(!output.contains("Total usage"));
    assert!(!output.contains("累计用量"));
    assert!(!output.contains("chars"));
    assert!(!output.contains("字符"));
    assert!(output.contains("token"));
    assert!(!output.contains("12k"));
    assert!(!output.contains("128k"));
    assert!(!output.contains("Session ID") && !output.contains("会话 ID"));
    assert!(!output.contains("default"));
    assert!(output.contains("Turn") || output.contains("本轮"));
    assert!(output.contains("75.0%"));
    // 上下行 token 使用简洁的方向箭头并留出间隔，不再依赖 Nerd Font 私有区图标
    assert!(output.contains("↑ 8.0k"));
    assert!(output.contains("↓ 4.0k"));
    assert!(!output.contains('\u{f090}'));
    assert!(!output.contains('\u{f08b}'));
    // 低压力时占比随标签弱化，不抢正文注意力
    assert!(output.contains("\x1b[2m(0.8%)"));
    assert!(output.contains("12") || output.contains("s") || output.contains("秒"));
    assert!(!output.contains("12.5"));
    assert!(!output.contains("Checkpoint"));
    assert!(!output.contains("Compaction"));
    // 水平 turn 分隔：始终换行全宽，绝不用竖线 │，也不再同行右接（避免 TUI 折行溢出）
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);
    assert!(
        !plain.contains('│'),
        "overview must not use a vertical pipe divider: {plain}"
    );
    let first = plain.lines().next().unwrap_or("");
    assert!(
        !first.contains('─'),
        "overview rule must not sit on the summary text line: {plain}"
    );
    let has_next_line = plain.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && trimmed.chars().all(|ch| ch == '─')
    });
    assert!(
        has_next_line,
        "overview should carry a next-line horizontal turn rule: {plain}"
    );

    // TUI 对齐后，纯 `─` 分隔行仍须顶格，否则会折行破坏 turn 分隔
    for line in output.lines() {
        let plain_line = strip_ansi_for_test(line);
        let trimmed = plain_line.trim();
        if trimmed.len() >= 3 && trimmed.chars().all(|ch| ch == '─') {
            let aligned = align_to_guide_column(line);
            assert_eq!(
                strip_ansi_for_test(&aligned),
                trimmed,
                "turn rule must stay flush after align: {aligned:?}"
            );
        }
    }
}

/// 验证上下文占比接近上限时数值升级为警示色。
#[test]
fn context_ratio_escalates_color_under_pressure() {
    let mut snapshot = SessionSnapshot {
        session_id: "default".to_string(),
        turn_count: 1,
        context_chars: 0,
        context_limit_chars: 0,
        context_ratio: 0.0,
        context_prompt_tokens: 900_000,
        context_window_tokens: 1_000_000,
        context_token_ratio: 0.9,
        checkpoint_count: 0,
        checkpoint_covered_turns: 0,
        tail_turns: 1,
        latest_checkpoint_at: None,
        latest_checkpoint_reason: None,
        usage: UsageSnapshot::default(),
        compaction: None,
        recovery: crate::state::RecoverySnapshot::default(),
        context_epoch: None,
        session_memory: None,
        tool_history: ToolHistorySummary::default(),
        runtime_recovery: crate::runtime_recovery::RuntimeRecoverySummary::default(),
        dynamic_sources: Vec::new(),
        projection_warnings: Vec::new(),
        active_run: None,
        last_turn_duration_ms: 0,
    };

    assert!(render_session_summary(&snapshot).contains("\x1b[31m(90.0%)"));

    snapshot.context_prompt_tokens = 700_000;
    snapshot.context_token_ratio = 0.7;
    assert!(render_session_summary(&snapshot).contains("\x1b[33m(70.0%)"));
}

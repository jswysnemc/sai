use super::session_summary::render_session_summary;
use crate::llm::Usage;
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
    assert!(output.contains('\u{f090}'));
    assert!(output.contains('\u{f08b}'));
    assert!(output.contains("12") || output.contains("s") || output.contains("秒"));
    assert!(!output.contains("12.5"));
    assert!(!output.contains("Checkpoint"));
    assert!(!output.contains("Compaction"));
}

use super::session_summary::{
    format_tokens_per_sec, format_ttft_ms, refit_turn_rule, render_session_summary_with,
};
use super::session_summary_labels::{ICON_LABELS, TEXT_LABELS};
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
        last_turn_ttft_ms: 420,
    };

    let output = render_session_summary_with(&snapshot, TEXT_LABELS);
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);

    // 行首是与助手正文同款的引导点
    assert!(output.starts_with("\x1b[2m•\x1b[0m "));
    // 顺序：总耗时 · 完成时刻 · 首字 · 输入 · 输出 · 缓存 · 速度
    assert!(plain.starts_with("• Worked for 12s · "), "{plain}");
    let clock = plain.split(" · ").nth(1).unwrap_or_default();
    assert!(clock.ends_with("AM") || clock.ends_with("PM"), "{plain}");
    assert!(
        plain.contains(
            " · First word 420ms · ↑ 8.0k toks · ↓ 4.0k toks · cached 75% · speed 320 toks/s"
        ),
        "{plain}"
    );
    // 上下文占用不在这一行；不出现会话 ID 与累计用量
    assert!(
        !plain.contains("/1M") && !plain.contains("Context"),
        "{plain}"
    );
    assert!(!plain.contains("default") && !plain.contains("Total usage"));
    assert!(!plain.contains("Compaction"));

    // 图标形态：箭头、缓存、速度换成 Nerd Font 图标，其余文字不变
    let icons = crate::render::activity_animation::strip_ansi_for_test(
        &render_session_summary_with(&snapshot, ICON_LABELS),
    );
    assert!(
        icons.contains("\u{f062} 8.0k toks · \u{f063} 4.0k toks"),
        "{icons}"
    );
    assert!(
        icons.contains("\u{f0737} 75% · \u{f04c5} 320 toks/s"),
        "{icons}"
    );
    // 数值保持正文色，不带青、绿色
    let colored = render_session_summary_with(&snapshot, ICON_LABELS);
    assert!(
        !colored.contains("\x1b[36m") && !colored.contains("\x1b[32m"),
        "{colored:?}"
    );
    // 总览本身不附带通栏横线，横线由 TUI 渲染层按宽度补上
    assert!(!plain.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && trimmed.chars().all(|ch| ch == '─')
    }));

    // 旧会话若已烘焙通栏线，显示时剥离，不再按宽度重画
    let legacy = format!("{output}\n\x1b[2m{}\x1b[0m", "─".repeat(40));
    let cleaned = refit_turn_rule(&legacy, 60);
    let cleaned_plain = crate::render::activity_animation::strip_ansi_for_test(&cleaned);
    assert!(
        !cleaned_plain.lines().any(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && trimmed.chars().all(|ch| ch == '─')
        }),
        "legacy turn rule must be stripped: {cleaned_plain}"
    );
}

#[test]
fn formats_ttft_and_output_rate() {
    assert_eq!(format_ttft_ms(420), "420ms");
    assert_eq!(format_ttft_ms(1_200), "1.2s");
    assert_eq!(format_tokens_per_sec(4_000, 12_500).as_deref(), Some("320"));
    assert_eq!(format_tokens_per_sec(20, 5_000).as_deref(), Some("4.0"));
    assert_eq!(format_tokens_per_sec(0, 5_000), None);
}

/// 恢复的总览用持久化的耗时、用量与完成时间重建。
#[test]
fn restored_turn_summary_rebuilds_from_persisted_metrics() {
    let usage = Usage {
        prompt_tokens: 5_800,
        completion_tokens: 254,
        total_tokens: 6_054,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    };
    let line = super::session_summary::render_history_turn_summary(
        87,
        6_100,
        Some(&usage),
        "2026-05-01T05:58:00Z",
    )
    .expect("persisted metrics must rebuild a summary");
    let plain = crate::render::activity_animation::strip_ansi_for_test(&line);
    assert!(plain.contains("Worked for 6.2s"), "{plain}");
    assert!(
        plain.contains("5.8k toks") && plain.contains("254 toks"),
        "{plain}"
    );
    // 完成时刻取自持久化时间并换算为本地 12 小时制
    let clock = plain.split(" · ").nth(1).unwrap_or_default();
    assert!(clock.ends_with("AM") || clock.ends_with("PM"), "{plain}");
    // 时间无法解析时省略时刻，其余照常
    let untimed = super::session_summary::render_history_turn_summary(87, 6_100, Some(&usage), "")
        .expect("summary without clock");
    assert!(!crate::render::activity_animation::strip_ansi_for_test(&untimed).contains(" PM"));
    assert!(super::session_summary::render_history_turn_summary(0, 0, None, "").is_none());
}

/// 总览嵌进分割线：信息靠左，横线向右补满。
#[test]
fn turn_rule_keeps_stats_left_and_fills_right() {
    let line = super::session_summary::inline_turn_rule("\x1b[2m•\x1b[0m stats", 20);
    let plain = crate::render::activity_animation::strip_ansi_for_test(&line);
    assert_eq!(plain, format!("stats {}", "─".repeat(14)));
    // 宽度不足时只保留信息，不画残缺横线
    let narrow = super::session_summary::inline_turn_rule("\x1b[2m•\x1b[0m stats", 10);
    assert_eq!(
        crate::render::activity_animation::strip_ansi_for_test(&narrow),
        "stats"
    );
}


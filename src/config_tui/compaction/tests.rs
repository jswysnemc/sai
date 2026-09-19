use super::{draft::*, view::*};
use crate::state::CompactionBudgetPolicy;
use crossterm::event::KeyCode;

/// 【上下文】【策略测试】草稿预览复用实际策略，保存前不提交
#[test]
fn draft_preview_matches_runtime_and_rejects_invalid_values() {
    let policy = CompactionBudgetPolicy::DEFAULT;
    let mut draft = Draft::new(policy, policy);
    draft.handle(KeyCode::Enter);
    draft.handle(KeyCode::Delete);
    for ch in "85%".chars() {
        draft.handle(KeyCode::Char(ch));
    }
    assert_eq!(draft.policy, policy);
    assert_eq!(draft.preview().unwrap().trigger_chars(128_000), 108_800);
    draft.handle(KeyCode::Enter);
    draft.handle(KeyCode::Down);
    draft.handle(KeyCode::Enter);
    draft.handle(KeyCode::Delete);
    for ch in "8k".chars() {
        draft.handle(KeyCode::Char(ch));
    }
    assert_eq!(draft.preview().unwrap().trigger_chars(128_000), 120_000);
    draft.handle(KeyCode::Enter);
    assert_eq!(
        draft.handle(KeyCode::Char('s')),
        Some(Outcome::Save(CompactionBudgetPolicy::from_context(
            0.85, 8_000
        )))
    );
    draft.handle(KeyCode::Enter);
    draft.input = Some("-1".to_string());
    assert!(draft.preview().is_err());
    assert_eq!(draft.handle(KeyCode::Enter), None);
    assert!(draft.error.is_some());
    assert!(draft.input.is_some());
}

/// 【上下文】【策略测试】恢复默认与取消均需显式保存才产生写入操作
#[test]
fn reset_and_cancel_are_transactional() {
    let policy = CompactionBudgetPolicy::from_context(0.75, 8_000);
    let mut draft = Draft::new(policy, CompactionBudgetPolicy::DEFAULT);
    draft.handle(KeyCode::Char('r'));
    assert!(draft.reset);
    assert_eq!(draft.handle(KeyCode::Char('s')), Some(Outcome::Reset));
    draft.handle(KeyCode::Left);
    assert!(!draft.reset);
    assert_eq!(draft.handle(KeyCode::Esc), Some(Outcome::Cancel));
    draft.handle(KeyCode::Enter);
    draft.input = Some("99".to_string());
    draft.handle(KeyCode::Esc);
    assert_eq!(draft.policy.ratio, 0.89);
}

/// 【上下文】【策略测试】关闭预留和大于窗口的预留都回到比例规则
#[test]
fn presets_and_bounds_match_policy() {
    let mut draft = Draft::new(
        CompactionBudgetPolicy::DEFAULT,
        CompactionBudgetPolicy::DEFAULT,
    );
    draft.selected = 1;
    draft.handle(KeyCode::Char('0'));
    assert_eq!(draft.policy.trigger_chars(1_000_000), 900_000);
    draft.handle(KeyCode::Char('3'));
    assert_eq!(draft.policy.trigger_chars(80_000), 72_000);
    draft.selected = 0;
    for _ in 0..100 {
        draft.handle(KeyCode::Left);
    }
    assert_eq!(draft.policy.ratio, 0.5);
    for _ in 0..100 {
        draft.handle(KeyCode::Right);
    }
    assert_eq!(draft.policy.ratio, 0.99);
}

/// 【上下文】【策略测试】不同终端宽度下保留触发说明且所有行符合宽度
#[test]
fn preview_wraps_for_narrow_terminals() {
    let draft = Draft::new(
        CompactionBudgetPolicy::DEFAULT,
        CompactionBudgetPolicy::DEFAULT,
    );
    let context = PreviewContext {
        window: 1_000_000,
        used: Some(100_000),
        scope: "session".to_string(),
        session: true,
    };
    for width in [20, 36, 60, 80] {
        let (lines, selected) = content(&draft, &context, width);
        assert!(lines
            .iter()
            .all(|line| crate::config_tui::ui::display_width(line) <= width));
        assert!(lines[selected].starts_with('>'));
        assert!(lines.join("").contains("950,000"));
    }
}

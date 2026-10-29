use super::{card, draft::*, view::*};
use crate::{config::CompactionMode, state::CompactionBudgetPolicy};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// 【上下文】【模式草稿】切换与步进直接改变唯一条件，确认后才生成保存动作。
#[test]
fn mode_switch_and_presets_control_single_trigger() {
    let mut draft = Draft::new(
        CompactionBudgetPolicy::DEFAULT,
        CompactionBudgetPolicy::DEFAULT,
    );
    assert_eq!(draft.preview().unwrap().trigger_chars(1_000_000), 900_000);
    draft.handle(KeyCode::Tab);
    assert_eq!(draft.preview().unwrap().trigger_chars(1_000_000), 950_000);
    draft.handle(KeyCode::Right);
    assert_eq!(draft.preview().unwrap().trigger_chars(1_000_000), 900_000);
    draft.handle(KeyCode::Left);
    draft.handle(KeyCode::Char('1'));
    assert_eq!(draft.preview().unwrap().trigger_chars(1_000_000), 992_000);
    assert_eq!(
        draft.handle(KeyCode::Enter),
        Some(Outcome::Save(
            CompactionBudgetPolicy::from_context(0.9, 8_000).with_mode(CompactionMode::Reserve)
        ))
    );
    draft.handle(KeyCode::Tab);
    for _ in 0..100 {
        draft.handle(KeyCode::Left);
    }
    assert_eq!(draft.preview().unwrap().trigger_chars(100_000), 50_000);
    for _ in 0..100 {
        draft.handle(KeyCode::Right);
    }
    assert_eq!(draft.preview().unwrap().trigger_chars(100_000), 99_000);
    draft.handle(KeyCode::Tab);
    for reserve in [0, 100_000, 200_000] {
        draft.policy.reserve_tokens = reserve;
        assert_eq!(draft.preview().unwrap().trigger_chars(100_000), 100_000);
    }
}

/// 【上下文】【输入编辑】普通数值不抢占保存，e 进入编辑，回车应用但不保存。
#[test]
fn numeric_editing_preserves_save_and_undo() {
    let mut draft = Draft::new(
        CompactionBudgetPolicy::DEFAULT,
        CompactionBudgetPolicy::DEFAULT,
    );
    draft.handle(KeyCode::Char('e'));
    for ch in "85%".chars() {
        draft.handle(KeyCode::Char(ch));
    }
    assert_eq!(draft.preview().unwrap().trigger_chars(100_000), 85_000);
    assert_eq!(draft.handle(KeyCode::Enter), None);
    draft.handle(KeyCode::Tab);
    draft.handle(KeyCode::Char('e'));
    for ch in "8k".chars() {
        draft.handle(KeyCode::Char(ch));
    }
    assert_eq!(draft.handle(KeyCode::Enter), None);
    assert_eq!(
        draft.handle(KeyCode::Char('s')),
        Some(Outcome::Save(
            CompactionBudgetPolicy::from_context(0.85, 8_000).with_mode(CompactionMode::Reserve)
        ))
    );
    draft.handle(KeyCode::Char('e'));
    draft.input = Some("-1".into());
    draft.handle(KeyCode::Enter);
    assert!(draft.error.is_some() && draft.input.is_some());
    draft.handle(KeyCode::Esc);
    assert!(draft.input.is_none());
    assert_eq!(
        draft.handle_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Some(Outcome::Cancel)
    );
}

/// 【上下文】【恢复默认】恢复是草稿操作，仍需用户明确保存。
#[test]
fn reset_and_cancel_are_transactional() {
    let mut draft = Draft::new(
        CompactionBudgetPolicy::from_context(0.75, 8_000).with_mode(CompactionMode::Reserve),
        CompactionBudgetPolicy::DEFAULT,
    );
    assert_eq!(draft.selected, 1);
    draft.handle(KeyCode::Char('r'));
    assert!(draft.reset);
    assert_eq!(draft.selected, 0);
    assert_eq!(draft.preview().unwrap().trigger_chars(1_000_000), 950_000);
    assert_eq!(draft.handle(KeyCode::Char('s')), Some(Outcome::Reset));
    draft.handle(KeyCode::Left);
    assert!(!draft.reset);
    assert_eq!(draft.handle(KeyCode::Esc), Some(Outcome::Cancel));
}

/// 【上下文】【响应式卡片】宽窄终端均不越界，预览显示真实的单条件阈值。
#[test]
fn preview_wraps_and_shows_selected_trigger() {
    let mut draft = Draft::new(
        CompactionBudgetPolicy::DEFAULT,
        CompactionBudgetPolicy::DEFAULT,
    );
    let context = PreviewContext {
        window: 1_000_000,
        used: Some(100_000),
        scope: "session".into(),
    };
    for width in [1, 12, 20, 36, 60, 80, 100] {
        for mode in [CompactionMode::Ratio, CompactionMode::Reserve] {
            draft.selected = usize::from(mode == CompactionMode::Reserve);
            let lines = frame(&draft, &context, width);
            assert!(
                lines
                    .iter()
                    .all(|line| crate::render::ansi_clip::ansi_visible_width(line) <= width),
                "width {width}: {lines:?}"
            );
            let text = crate::render::ansi_clip::strip_ansi(&lines.join("\n"));
            let compact_text = text.replace('\n', "");
            assert!(
                compact_text.contains(if mode == CompactionMode::Ratio {
                    "900,000"
                } else {
                    "950,000"
                }),
                "{text}"
            );
        }
    }
    assert!(card::fit_height(frame(&draft, &context, 80), 4).len() <= 4);
    for selected in [0, 1] {
        draft.selected = selected;
        assert_eq!(frame(&draft, &context, 100).len(), 5);
        let narrow = crate::render::ansi_clip::strip_ansi(&frame(&draft, &context, 36).join("\n"));
        assert!(narrow.contains("[1.0M]"), "{narrow}");
        assert!(
            narrow.lines().all(|line| line.starts_with(['╭', '│', '╰'])),
            "{narrow}"
        );
    }
}

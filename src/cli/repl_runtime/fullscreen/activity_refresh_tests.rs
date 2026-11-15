//! 刷新期间保持工具阅读位置和内外层跟随状态的回归。
use super::*;

/// 【终端】【收缩夹具】两个节点共用执行链，前置节点长度可变。
fn two_steps(prefix: usize, active: bool) -> FullscreenDocument {
    let mut document = source(prefix + 60);
    document.paragraphs[1].end = 2 + prefix;
    document.paragraphs.push(ParagraphSpan {
        key: ParagraphKey {
            cell: 3,
            part: if active {
                ParagraphPart::Active
            } else {
                ParagraphPart::Whole
            },
        },
        start: 2 + prefix,
        end: 2 + prefix + 60,
        control: 2 + prefix,
        expanded: true,
    });
    for row in 0..60 {
        document.lines[2 + prefix + row] = AnsiLine::new(format!("READ-{row:02}"));
    }
    document
}

/// 【终端】【同宽刷新】前置内容收缩及工具完成都不能改变正在阅读的节点正文。
#[test]
fn same_width_refresh_preserves_node_when_preceding_content_shrinks() {
    let mut views = ActivityViewports::default();
    views.update(two_steps(25, true), 30, 80, 30);
    views.scroll(GROUP, -20);
    let before = views.project();
    let read = before.lines[3].clone();
    assert!(read.as_str().starts_with("READ-"));
    let after = views.update(two_steps(3, false), 30, 80, 30);
    assert_eq!(after.lines[3], read);
    assert!(!views.is_following(GROUP));
}

/// 【终端】【短暂触底】刷新收缩使偏移夹紧到末尾，不等于用户恢复跟随。
#[test]
fn temporary_shrink_does_not_resume_activity_follow() {
    let mut views = ActivityViewports::default();
    views.update(source(60), 30, 80, 30);
    views.scroll(GROUP, -10);
    views.update(source(3), 30, 80, 30);
    assert!(!views.is_following(GROUP));
    let grown = views.update(source(70), 30, 80, 30);
    assert!(!views.is_following(GROUP));
    assert!(grown.lines.iter().any(|line| line.as_str() == "STEP-2"));
    assert!(!grown.lines.iter().any(|line| line.as_str() == "STEP-69"));
    views.scroll(GROUP, isize::MAX);
    assert!(views.is_following(GROUP));
}

/// 【终端】【外层阅读】滚轮、边界滚轮和重复选择都暂停主会话跟随，新回答只提示。
#[test]
fn inner_reading_is_not_displaced_by_new_answer() {
    for interaction in 0..3 {
        let mut state = FullscreenState::new();
        let document = state.activity.update(source(60), 30, 80, 30);
        state.apply_document(document, 30);
        let outer = state.scroll;
        match interaction {
            0 => {
                state.wheel_at(Some(3), -3, 30);
            }
            1 => {
                state.wheel_at(Some(3), 3, 30);
            }
            _ => {
                state.focus_activity_at(3);
                state.focus_activity_at(3);
            }
        }
        assert!(!state.follow);
        let mut next = source(70);
        next.lines
            .extend((0..20).map(|row| AnsiLine::new(format!("NEW-ANSWER-{row}"))));
        let document = state.activity.update(next, 30, 80, 30);
        state.apply_document(document, 30);
        assert_eq!(state.scroll, outer);
        assert!(!state.follow);
        assert!(state.unseen);
    }
}

/// 【终端】【外层收缩】短暂变成一屏也不能在下一帧追随新回答。
#[test]
fn temporary_shrink_does_not_resume_outer_follow() {
    let mut state = FullscreenState::new();
    state.apply_document(source(60), 20);
    state.scroll_by(-10, 20);
    state.apply_document(source(3), 20);
    assert!(!state.follow);
    state.apply_document(source(80), 20);
    assert_eq!(state.scroll, 0);
    assert!(!state.follow);
}

/// 【终端】【键盘阅读】链内定位完整可见时也不能因触及主会话底部恢复外层跟随。
#[test]
fn inner_keyboard_reveal_keeps_outer_follow_paused() {
    let mut state = FullscreenState::new();
    let document = state.activity.update(source(60), 30, 80, 30);
    state.apply_document(document, 30);
    for navigation in [ActivityNavigation::Top, ActivityNavigation::Latest] {
        state.navigate_activity(navigation, 30);
        assert!(!state.follow);
    }
}

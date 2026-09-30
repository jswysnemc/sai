use super::*;
use crate::render::transcript::{AnsiLine, FullscreenDocument};

/// 构造 100 行文档：第 10、50、80 行是用户消息，第 20 行起有一段 5 行的折叠段。
fn document(extra: usize) -> FullscreenDocument {
    let mut document = FullscreenDocument {
        lines: (0..100 + extra)
            .map(|index| AnsiLine::new(format!("row-{index}")))
            .collect(),
        ..FullscreenDocument::default()
    };
    for row in [10, 50, 80] {
        document
            .anchors
            .push(crate::render::transcript::UserAnchor {
                row,
                summary: format!("message at {row}"),
            });
    }
    document
        .paragraphs
        .push(crate::render::transcript::ParagraphSpan {
            key: 7,
            start: 20,
            end: 25,
            expanded: false,
        });
    document
}

/// 跟随时贴底；离开底部后新输出只提示不跳动，回到底部恢复跟随。
#[test]
fn follow_sticks_to_bottom_until_user_scrolls() {
    let mut state = FullscreenState::new();
    state.apply_document(document(0), 20);
    assert_eq!(state.scroll, 80);
    assert!(state.scroll_by(-10, 20));
    assert!(!state.follow);
    state.apply_document(document(5), 20);
    assert_eq!(state.scroll, 70, "离开底部后位置不变");
    assert!(state.unseen);
    state.scroll_to(usize::MAX, 20);
    assert!(state.follow && !state.unseen);
    assert_eq!(state.scroll, 85);
}

/// Alt+↑↓ 在用户消息之间跳转，越过末条后到底。
#[test]
fn relative_jumps_walk_user_messages() {
    let mut state = FullscreenState::new();
    state.apply_document(document(0), 20);
    state.scroll_to(0, 20);
    state.jump_relative(true, 20);
    assert_eq!(state.scroll, 9);
    assert_eq!(state.current_anchor(), Some(0));
    state.jump_relative(true, 20);
    assert_eq!(state.current_anchor(), Some(1));
    state.jump_relative(false, 20);
    assert_eq!(state.current_anchor(), Some(0));
    state.jump_to_anchor(2, 20);
    state.jump_relative(true, 20);
    assert!(state.follow);
}

/// 点击折叠段任意行展开；展开后只有标题行能收起，重排后标题行停在原屏幕位置。
#[test]
fn toggling_keeps_the_paragraph_header_in_place() {
    let mut state = FullscreenState::new();
    state.apply_document(document(0), 20);
    state.scroll_to(15, 20);
    let (key, offset) = state.toggle_at(22).expect("折叠段应可展开");
    assert_eq!((key, offset), (7, 5));
    assert!(state.expanded.contains(&7));
    // 模拟重排：展开后段落变长，前面插入了 3 行
    let mut next = document(30);
    next.paragraphs[0] = crate::render::transcript::ParagraphSpan {
        key: 7,
        start: 23,
        end: 55,
        expanded: true,
    };
    state.apply_document(next, 20);
    state.restore_toggle_anchor(key, offset, 20);
    assert_eq!(state.scroll, 18);
    assert!(state.toggle_at(30).is_none(), "展开段正文不应收起");
    assert!(state.toggle_at(23).is_some());
    assert!(!state.expanded.contains(&7));
}

/// 文档里已不存在的展开键被清理。
#[test]
fn stale_expanded_keys_are_dropped() {
    let mut state = FullscreenState::new();
    state.expanded.insert(99);
    state.apply_document(document(0), 20);
    assert!(state.expanded.is_empty());
}

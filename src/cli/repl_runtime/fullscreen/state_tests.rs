use super::*;
use crate::render::transcript::{AnsiLine, FullscreenDocument, ParagraphKey};

/// 测试折叠段的段落键。
const K7: ParagraphKey = ParagraphKey {
    cell: 7,
    part: crate::render::transcript::ParagraphPart::Whole,
};

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
            key: K7,
            start: 20,
            end: 25,
            expanded: false,
            control: 22,
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

/// 只有折叠提示行能展开；重排后标题行停在原屏幕位置；展开后点末尾收起行收起。
#[test]
fn toggling_keeps_the_paragraph_header_in_place() {
    let mut state = FullscreenState::new();
    state.apply_document(document(0), 20);
    state.scroll_to(15, 20);
    assert!(state.toggle_at(21).is_none(), "折叠段正文不响应点击");
    let (key, offset) = state.toggle_at(22).expect("折叠提示应可展开");
    assert_eq!((key, offset), (K7, 5));
    assert!(state.expanded.contains(&K7));
    // 模拟重排：展开后段落变长，前面插入了 3 行
    let mut next = document(30);
    next.paragraphs[0] = crate::render::transcript::ParagraphSpan {
        key: K7,
        start: 23,
        end: 55,
        expanded: true,
        control: 54,
    };
    state.apply_document(next, 20);
    state.restore_toggle_anchor(key, offset, 20);
    assert_eq!(state.scroll, 18);
    assert!(state.toggle_at(30).is_none(), "展开段正文不响应点击");
    assert!(state.toggle_at(54).is_some(), "点收起行收起");
    assert!(!state.expanded.contains(&K7));
    assert!(state.toggle_at(99).is_none(), "段落范围外的行不触发切换");
}

/// 在长段正文中部收起时标题已在视野上方，收起后段落回到正文顶部。
#[test]
fn collapsing_from_the_middle_keeps_the_paragraph_visible() {
    let mut state = FullscreenState::new();
    let mut open = document(30);
    open.paragraphs[0] = crate::render::transcript::ParagraphSpan {
        key: K7,
        start: 20,
        end: 55,
        expanded: true,
        control: 54,
    };
    state.expanded.insert(K7);
    state.apply_document(open, 20);
    state.scroll_to(40, 20);
    let (key, offset) = state.toggle_at(54).expect("收起行应能收起");
    assert!(offset < 0, "标题行已滚出正文顶部");
    state.apply_document(document(0), 20);
    state.restore_toggle_anchor(key, offset, 20);
    assert_eq!(state.scroll, 20, "收起后的段落标题停在正文顶部");
}

/// 文档里已不存在的展开键被清理。
#[test]
fn stale_expanded_keys_are_dropped() {
    let mut state = FullscreenState::new();
    state.expanded.insert(ParagraphKey::whole(99));
    state.apply_document(document(0), 20);
    assert!(state.expanded.is_empty());
}

/// 鼠标停在折叠段上时报告悬停段落；滚动后按新位置重新判断，拖选时不显示悬停。
#[test]
fn hovered_paragraph_follows_pointer_and_scroll() {
    let mut state = FullscreenState::new();
    state.apply_document(document(0), 20);
    state.scroll_to(15, 20);
    state.pointer_row = Some(6);
    assert!(state.hovered_paragraph().is_none(), "折叠段正文不提亮");
    state.pointer_row = Some(7);
    assert_eq!(state.hovered_paragraph().map(|span| span.key), Some(K7));
    state.scroll_to(30, 20);
    assert!(
        state.hovered_paragraph().is_none(),
        "滚动后指针下已不是折叠段"
    );
    state.scroll_to(15, 20);
    state.pointer_row = Some(7);
    state.press = Some(super::super::selection::TextPoint { row: 21, col: 0 });
    assert!(state.hovered_paragraph().is_none(), "按下拖选时不显示悬停");
}

/// 回到底部按钮与折叠提示同在最后一行时，指针落在按钮上只算按钮悬停。
#[test]
fn bottom_button_takes_hover_priority_over_fold_hint() {
    let mut state = FullscreenState::new();
    state.apply_document(document(0), 20);
    // 让折叠提示（第 22 行）落在正文最后一行，且不在底部
    state.scroll_to(3, 20);
    assert!(!state.follow);
    state.pointer_row = Some(19);
    state.pointer_col = Some(40);
    assert!(state.hovered_paragraph().is_some());
    assert!(super::super::bottom_button::is_hovered(&state, 80, 20));
    state.pointer_col = Some(2);
    assert!(!super::super::bottom_button::is_hovered(&state, 80, 20));
}

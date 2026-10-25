use super::*;
use crate::cli::repl_runtime::fullscreen::state::{ActivityNavigation, FullscreenState};
use crate::render::transcript::{ActivityRegion, ParagraphPart, ParagraphSpan, UserAnchor};

const GROUP: ParagraphKey = ParagraphKey {
    cell: 1,
    part: ParagraphPart::ActivityGroup,
};
const STEP: ParagraphKey = ParagraphKey {
    cell: 2,
    part: ParagraphPart::Whole,
};

/// 【终端】【摘要组测试】没有展开详情的组保留自然高度，滚轮和翻页交回普通会话。
/// 参数: 无；返回: 无。
#[test]
fn summary_only_group_bypasses_nested_viewport_and_scroll() {
    let mut document = source(12);
    document.activities[0].scrollable = false;
    document.activities[0].active = false;
    let expected = document.lines.clone();
    let mut state = FullscreenState::new();
    let projected = state.activity.update(document, 30, 80);
    assert_eq!(projected.lines, expected);
    assert_eq!(projected.activities[0].footer, None);
    state.apply_document(projected, 8);
    assert_eq!(state.scroll_activity_at(3, -3), None);
    assert_eq!(
        state.navigate_activity(ActivityNavigation::Page(false), 8),
        None
    );
}

/// 【终端】【子视口测试】构造前后带普通消息的长执行链。
/// 参数: `count` 为链内行数；返回: 含可点击步骤与用户锚点的源文档。
fn source(count: usize) -> FullscreenDocument {
    let mut document = FullscreenDocument {
        lines: vec![AnsiLine::new("USER".into()), AnsiLine::new("GROUP".into())],
        ..FullscreenDocument::default()
    };
    document
        .lines
        .extend((0..count).map(|index| AnsiLine::new(format!("STEP-{index}"))));
    document.lines.push(AnsiLine::new("ANSWER".into()));
    document.lines.push(AnsiLine::new("NEXT-USER".into()));
    document.activities.push(ActivityRegion {
        key: GROUP,
        start: 1,
        end: count + 2,
        scrollable: true,
        active: true,
        footer: None,
    });
    document.paragraphs.push(ParagraphSpan {
        key: GROUP,
        start: 1,
        end: 2,
        expanded: true,
        control: 1,
    });
    document.paragraphs.push(ParagraphSpan {
        key: STEP,
        start: 2,
        end: count + 2,
        expanded: true,
        control: 2,
    });
    document.anchors.push(UserAnchor {
        row: 0,
        summary: "USER".into(),
    });
    document.anchors.push(UserAnchor {
        row: count + 3,
        summary: "NEXT-USER".into(),
    });
    document
}

/// 【终端】【子视口测试】输出增长仅改变链内内容，标题、回答和输入区上方坐标保持稳定。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_has_fixed_height_and_follows_latest() {
    let mut views = ActivityViewports::default();
    let first = views.update(source(3), 30, 80);
    let next = views.update(source(40), 30, 80);
    assert_eq!(first.lines.len(), next.lines.len());
    assert_eq!(first.activities[0].start, next.activities[0].start);
    assert_eq!(first.activities[0].end, next.activities[0].end);
    assert_eq!(next.lines[1].as_str(), "GROUP");
    assert_eq!(next.lines[2].as_str(), "STEP-0");
    assert_eq!(next.lines[3].as_str(), "STEP-31");
    assert_eq!(next.lines[11].as_str(), "STEP-39");
    assert!(next.lines[12].as_str().contains("32–40/40"));
    assert_eq!(next.anchors[1].row, 14);
    assert_eq!(next.lines[13].as_str(), "ANSWER");
}

/// 【终端】【子视口测试】链内滚轮暂停独立跟随，边界不把事件转给整页滚动。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_wheel_does_not_move_outer_scroll() {
    let mut state = FullscreenState::new();
    let document = state.activity.update(source(40), 30, 80);
    state.apply_document(document, 8);
    let outer = state.scroll;
    assert_eq!(state.scroll_activity_at(3, -3), Some(true));
    assert_eq!(state.scroll, outer);
    assert_eq!(state.document.lines[3].as_str(), "STEP-28");
    let document = state.activity.update(source(45), 30, 80);
    state.apply_document(document, 8);
    assert_eq!(state.document.lines[3].as_str(), "STEP-28");
    assert_eq!(state.scroll_activity_at(3, -100), Some(true));
    assert_eq!(state.scroll_activity_at(3, -3), Some(false));
    assert_eq!(state.scroll, outer);
    assert_eq!(state.scroll_activity_at(0, -3), None);
    assert_eq!(state.scroll_activity_at(3, 100), Some(true));
    let document = state.activity.update(source(50), 30, 80);
    assert_eq!(document.lines[3].as_str(), "STEP-41");
}

/// 【终端】【子视口测试】保留标题和正文使用同一点击范围，避免点击正文后外层偏移。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_maps_clicks_and_preserves_hidden_keys() {
    let mut views = ActivityViewports::default();
    let document = views.update(source(40), 30, 80);
    assert_eq!(document.paragraph_at(2).unwrap().key, STEP);
    assert_eq!(document.paragraph_control_at(2).unwrap().key, STEP);
    assert_eq!(document.paragraph_at(3).unwrap().start, 2);
    assert_eq!(document.paragraph_at(3).unwrap().control, 2);
    assert!(document.retained_keys.contains(&STEP));
    views.focus(STEP);
    let focused = views.update(source(40), 30, 80);
    assert_eq!(focused.lines[2].as_str(), "STEP-0");
    assert_eq!(focused.paragraph_control_at(2).unwrap().key, STEP);
}

/// 【终端】【子视口测试】待审批分组不裁切，缩小终端后普通分组仍符合高度与宽度约束。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_keeps_permissions_visible_and_resizes() {
    let mut views = ActivityViewports::default();
    let mut pending = source(40);
    pending.activities[0].scrollable = false;
    let document = views.update(pending, 20, 80);
    assert_eq!(document.lines.len(), 44);
    let narrow = views.update(source(40), 10, 12);
    assert_eq!(narrow.activities[0].end - narrow.activities[0].start, 6);
    assert!(narrow.lines[6].as_str().contains("38–40/"));
    assert!(
        crate::render::activity_animation::strip_ansi_for_test(narrow.lines[6].as_str())
            .chars()
            .count()
            <= 12
    );
    assert_eq!(views.update(source(40), 3, 12).lines.len(), 44);
}

/// 【终端】【子视口测试】经典样式不创建留白和独立视口，清屏后丢弃旧链位置。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_leaves_classic_documents_unchanged() {
    let mut views = ActivityViewports::default();
    views.update(source(40), 30, 80);
    let mut classic = source(4);
    classic.activities.clear();
    let original = classic.lines.clone();
    assert_eq!(views.update(classic, 30, 80).lines, original);
    views.update(FullscreenDocument::default(), 30, 80);
    assert!(views.positions.is_empty());
}

/// 【终端】【子视口测试】多条执行链各自记录位置，滚动一条链不改变其他链。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_tracks_multiple_groups_independently() {
    let mut document = source(40);
    let offset = document.lines.len();
    let mut second = source(25);
    for region in &mut second.activities {
        region.key.cell += 10;
        region.start += offset;
        region.end += offset;
    }
    for span in &mut second.paragraphs {
        span.key.cell += 10;
        span.start += offset;
        span.end += offset;
        span.control += offset;
    }
    for anchor in &mut second.anchors {
        anchor.row += offset;
    }
    document.lines.extend(second.lines);
    document.activities.extend(second.activities);
    document.paragraphs.extend(second.paragraphs);
    document.anchors.extend(second.anchors);
    let mut views = ActivityViewports::default();
    let initial = views.update(document, 30, 80);
    let second_body = initial.activities[1].start + 1;
    let original = initial.lines[second_body + 1].clone();
    assert!(views.scroll(GROUP, -6));
    let next = views.project();
    assert_eq!(next.lines[3].as_str(), "STEP-25");
    assert_eq!(next.lines[second_body + 1], original);
    assert_eq!(initial.anchors, next.anchors);
}

/// 【终端】【子视口测试】完成后的短链恢复自然高度，不阻止外层滚轮。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_completed_short_group_uses_natural_height() {
    let mut state = FullscreenState::new();
    let mut completed = source(3);
    completed.activities[0].active = false;
    let original = completed.lines.clone();
    let document = state.activity.update(completed, 30, 80);
    state.apply_document(document, 30);
    assert_eq!(state.document.lines, original);
    assert_eq!(state.document.activities[0].footer, None);
    assert_eq!(state.scroll_activity_at(3, -3), None);
}

/// 【终端】【子视口测试】键盘浏览和提示行恢复跟随只影响目标链，不移动会话。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_keyboard_and_resume_keep_outer_position() {
    use crate::cli::repl_runtime::fullscreen::state::ActivityNavigation;
    let mut state = FullscreenState::new();
    let document = state.activity.update(source(40), 30, 80);
    state.apply_document(document, 20);
    let outer = state.scroll;
    assert_eq!(
        state.navigate_activity(ActivityNavigation::Page(false), 20),
        Some(true)
    );
    assert_eq!(state.document.lines[3].as_str(), "STEP-23");
    assert_eq!(
        state.navigate_activity(ActivityNavigation::Top, 20),
        Some(true)
    );
    assert_eq!(state.document.lines[2].as_str(), "STEP-0");
    let footer = state.document.activities[0].footer.unwrap();
    assert_eq!(state.resume_activity_at(footer), Some(true));
    assert_eq!(state.document.lines[3].as_str(), "STEP-31");
    assert_eq!(state.scroll, outer);
    assert_eq!(state.resume_activity_at(0), None);
    assert_eq!(
        state.navigate_activity(ActivityNavigation::Latest, 20),
        Some(false)
    );
}

/// 【终端】【子视口测试】键盘操作只选择可见执行链，不滚动不可见的历史链。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_keyboard_ignores_offscreen_groups() {
    use crate::cli::repl_runtime::fullscreen::state::ActivityNavigation;
    let mut state = FullscreenState::new();
    let mut document = source(40);
    document
        .lines
        .extend((0..50).map(|index| AnsiLine::new(format!("ANSWER-{index}"))));
    let projected = state.activity.update(document, 30, 80);
    state.apply_document(projected, 10);
    assert_eq!(state.navigate_activity(ActivityNavigation::Top, 10), None);
}

/// 【终端】【子视口测试】收起保留在顶部的标题后，分组标题与外层位置不发生跳动。
/// 参数: 无；返回: 无。
#[test]
fn activity_viewport_fold_keeps_group_anchor_instead_of_moving_conversation() {
    let mut state = FullscreenState::new();
    let document = state.activity.update(source(40), 30, 80);
    state.apply_document(document, 10);
    state.scroll_to(0, 10);
    state.expanded.insert(STEP);
    let (key, offset) = state.toggle_at(2).unwrap();
    let mut folded = source(2);
    folded.paragraphs[1].start = 3;
    folded.paragraphs[1].control = 3;
    folded.paragraphs[1].expanded = false;
    let document = state.activity.update(folded, 30, 80);
    state.apply_document(document, 10);
    state.restore_toggle_anchor(key, offset, 10);
    assert_eq!(state.document.activities[0].start, 1);
    assert_eq!(state.scroll, 0);
}

use super::*;

/// 【终端】【阶段位置测试】同一份短内容从运行切到完成不能改变视口高度或插拔页脚。
/// 参数: 无；返回: 无。
#[test]
fn short_activity_has_the_same_geometry_before_and_after_completion() {
    let mut views = ActivityViewports::default();
    let running = views.update(source(3), 34, 80, 40);
    let mut completed = source(3);
    completed.activities[0].active = false;
    let done = views.update(completed, 34, 80, 40);
    assert_eq!(running.lines, done.lines);
    assert_eq!(running.anchors, done.anchors);
    assert_eq!(running.activities[0].footer, None);
}

/// 【终端】【点击位置测试】展开详情只占用标题下方空间，不把已可见标题搬到页面顶部。
/// 参数: 无；返回: 无。
#[test]
fn expanding_history_preserves_outer_anchor_and_fits_remaining_space() {
    let mut state = FullscreenState::new();
    state.layout = Some(super::super::super::layout::FullscreenLayout::compute(
        84, 40, 5,
    ));
    let mut closed = padded_source(2);
    closed.paragraphs[1].expanded = false;
    closed.activities[0].active = false;
    closed.activities[0].scrollable = false;
    let document = state.activity.update(closed, 34, 80, 40);
    state.apply_document(document, 34);
    let group_row = state.document.activities[0].start - state.scroll;
    let (key, offset) = state.toggle_at(8).unwrap();
    let mut opened = padded_source(60);
    opened.activities[0].active = false;
    let document = state.activity.update(opened, 34, 80, 40);
    state.apply_document(document, 34);
    state.restore_toggle_anchor(key, offset, 34);
    let region = &state.document.activities[0];
    assert_eq!(region.start - state.scroll, group_row);
    assert!(region.end <= state.scroll + 34);
}

/// 【终端】【子标题位置测试】展开第二个节点时保留前一个摘要，不把点击的标题移动到链首。
/// 参数: 无；返回: 无。
#[test]
fn focused_child_keeps_its_offset_below_the_group() {
    let mut views = ActivityViewports::default();
    let mut document = source(40);
    document.activities[0].active = false;
    document.lines[2] = AnsiLine::new("OLDER".into());
    document.lines[3] = AnsiLine::new("TARGET".into());
    document.paragraphs[1].start = 3;
    document.paragraphs[1].control = 3;
    views.update(document.clone(), 34, 80, 40);
    views.focus_at(STEP, 1, Some(12));
    let focused = views.update(document, 34, 80, 40);
    assert_eq!(focused.lines[2].as_str(), "OLDER");
    assert_eq!(focused.lines[3].as_str(), "TARGET");
    assert_eq!(focused.activities[0].end - focused.activities[0].start, 14);
}

/// 【终端】【位置夹具】在执行链前放置普通会话内容，以复现页面中部的展开操作。
/// 参数: `count` 为链内行数；返回: 标题位于正文第七行的文档。
fn padded_source(count: usize) -> FullscreenDocument {
    let mut document = source(count);
    document
        .lines
        .splice(0..0, (0..6).map(|_| AnsiLine::new("CONTEXT".into())));
    for region in &mut document.activities {
        region.start += 6;
        region.end += 6;
    }
    for span in &mut document.paragraphs {
        span.start += 6;
        span.end += 6;
        span.control += 6;
    }
    for anchor in &mut document.anchors {
        anchor.row += 6;
    }
    document
}

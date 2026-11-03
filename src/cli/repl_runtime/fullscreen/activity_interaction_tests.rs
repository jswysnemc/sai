use super::runtime_tests::mouse;
use super::state::{ActivityNavigation, FullscreenState};
use crate::cli::repl_runtime::ReplRuntime;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::{
    ActivityRegion, AnsiLine, FullscreenDocument, ParagraphKey, ParagraphPart, ParagraphSpan,
    TranscriptRenderOptions, TranscriptStore,
};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use crossterm::event::MouseEventKind;

const GROUP: ParagraphKey = ParagraphKey {
    cell: 1,
    part: ParagraphPart::ActivityGroup,
};
const STEP: ParagraphKey = ParagraphKey {
    cell: 2,
    part: ParagraphPart::Active,
};

/// 【终端】【交互夹具】构造前后有会话正文的单节点执行链。
/// 参数: `prefix` 为链前正文行数，`count` 为节点正文行数；返回: 完整源文档。
fn source(prefix: usize, count: usize) -> FullscreenDocument {
    let mut document = FullscreenDocument::default();
    document
        .lines
        .extend((0..prefix).map(|row| AnsiLine::new(format!("CONTEXT-{row}"))));
    document.lines.push(AnsiLine::new("GROUP".into()));
    document.lines.push(AnsiLine::new("TOOL".into()));
    document
        .lines
        .extend((0..count).map(|row| AnsiLine::new(format!("DETAIL-{row}"))));
    let end = document.lines.len();
    document
        .lines
        .extend((0..40).map(|row| AnsiLine::new(format!("ANSWER-{row}"))));
    document.activities.push(ActivityRegion {
        key: GROUP,
        start: prefix,
        end,
        scrollable: true,
        active: false,
        footer: None,
    });
    document.paragraphs.push(ParagraphSpan {
        key: GROUP,
        start: prefix,
        end: prefix + 1,
        expanded: true,
        control: prefix,
    });
    document.paragraphs.push(ParagraphSpan {
        key: STEP,
        start: prefix + 1,
        end,
        expanded: true,
        control: prefix + 1,
    });
    document
}

/// 【终端】【事件夹具】把长执行链放入真实全屏鼠标处理入口。
/// 参数: 无；返回: 指针可命中执行链的运行时。
fn runtime() -> ReplRuntime {
    let mut runtime = ReplRuntime::new(
        1000,
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        },
    );
    runtime.enter_fullscreen().unwrap();
    let state = &mut runtime.fullscreen.as_mut().unwrap().state;
    let layout = state.layout.unwrap();
    let document = state.activity.update(
        source(20, 100),
        usize::from(layout.body_height),
        usize::from(layout.content_width),
        usize::from(layout.rows),
    );
    state.apply_document(document, usize::from(layout.body_height));
    state.scroll = 18;
    state.follow = false;
    runtime
}

/// 【终端】【直接滚动回归】悬停工具正文即可滚动内部，无须按键或预先单击。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_hover_scrolls_inside_without_modifiers() {
    let mut runtime = runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let before = state.scroll;
    let detail = state.document.lines[22].clone();
    let row = state.layout.unwrap().body_top + 4;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 8, row))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.scroll, before);
    assert_eq!(state.activity.selected, Some(GROUP));
    assert_ne!(state.document.lines[22], detail);
}

/// 【终端】【边界交接回归】上下边界先阻挡一次，再连续放行同向滚轮到主会话。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_boundary_blocks_once_then_scrolls_conversation() {
    for (kind, navigation, delta) in [
        (MouseEventKind::ScrollUp, ActivityNavigation::Top, -3),
        (MouseEventKind::ScrollDown, ActivityNavigation::Latest, 3),
    ] {
        let mut runtime = runtime();
        let state = &mut runtime.fullscreen.as_mut().unwrap().state;
        let layout = state.layout.unwrap();
        state.navigate_activity(navigation, usize::from(layout.body_height));
        let before = state.scroll;
        let row = layout.body_top + (state.document.activities[0].start + 8 - before) as u16;
        runtime
            .handle_fullscreen_event(&mouse(kind, 8, row))
            .unwrap();
        assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, before);
        for step in 1..=3 {
            runtime
                .handle_fullscreen_event(&mouse(kind, 8, row))
                .unwrap();
            assert_eq!(
                runtime.fullscreen.as_ref().unwrap().state.scroll,
                before.saturating_add_signed(delta * step),
                "边界放行后不能因外层位置变化再次阻挡同一执行链"
            );
        }
    }
}

/// 【终端】【剩余位移回归】内部距离边界不足一格时，本次滚轮不能把剩余位移交给外层。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_reaching_boundary_keeps_outer_scroll_still() {
    let mut runtime = runtime();
    let state = &mut runtime.fullscreen.as_mut().unwrap().state;
    assert!(state.activity.scroll(GROUP, -1));
    state.document = state.activity.project();
    let before = state.scroll;
    let detail = state.document.lines[22].clone();
    let row = state.layout.unwrap().body_top + 4;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.scroll, before);
    assert_ne!(state.document.lines[22], detail);
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, before);
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 3
    );
}

/// 【终端】【反向滚动回归】交给外层后反向滚动立即回到内部，下一次到边界重新阻挡。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_reversing_returns_to_inner_scroll() {
    let mut runtime = runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let before = state.scroll;
    let row = state.layout.unwrap().body_top + 4;
    for _ in 0..2 {
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
            .unwrap();
    }
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 3
    );
    let detail = runtime.fullscreen.as_ref().unwrap().state.document.lines[22].clone();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 8, row))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.scroll, before + 3);
    assert_ne!(state.document.lines[22], detail);
    for _ in 0..2 {
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
            .unwrap();
        assert_eq!(
            runtime.fullscreen.as_ref().unwrap().state.scroll,
            before + 3
        );
    }
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 6
    );
}

/// 【终端】【离开区域回归】指针离开正文后返回同一执行链，边界应重新阻挡一次。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_leaving_region_resets_boundary_handoff() {
    let mut runtime = runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let before = state.scroll;
    let layout = state.layout.unwrap();
    let row = layout.body_top + 4;
    for _ in 0..2 {
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
            .unwrap();
    }
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 3
    );
    for column in [layout.cols - 1, 8] {
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::Moved, column, row))
            .unwrap();
    }
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 3
    );
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 6
    );
}

/// 【终端】【区域内移动回归】指针仍在同一执行链内时，微小移动不重置边界交接。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_moving_inside_region_preserves_boundary_handoff() {
    let mut runtime = runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let before = state.scroll;
    let row = state.layout.unwrap().body_top + 4;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::Moved, 9, row + 1))
        .unwrap();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 9, row + 1))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 3
    );
}

/// 【终端】【目标切换回归】切换到另一条执行链时，不能沿用上一条链的边界放行状态。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_switching_chains_resets_boundary_handoff() {
    let mut runtime = runtime();
    let state = &mut runtime.fullscreen.as_mut().unwrap().state;
    let layout = state.layout.unwrap();
    // 1. 【终端】【双链夹具】使用较小详情预算，使两条独立执行链同时可见
    let mut document = source(20, 100);
    document.lines.truncate(document.activities[0].end);
    let offset = document.lines.len();
    let mut second = source(0, 100);
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
    document.lines.extend(second.lines);
    document.activities.extend(second.activities);
    document.paragraphs.extend(second.paragraphs);
    let document = state
        .activity
        .update(document, 10, usize::from(layout.content_width), 10);
    state.apply_document(document, usize::from(layout.body_height));
    let before = state.scroll;
    let first = layout.body_top + (state.document.activities[0].start + 1 - before) as u16;
    let second = layout.body_top + (state.document.activities[1].start + 1 - before) as u16;
    for row in [first, second, first] {
        let before = runtime.fullscreen.as_ref().unwrap().state.scroll;
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
            .unwrap();
        assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, before);
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
            .unwrap();
        assert_eq!(
            runtime.fullscreen.as_ref().unwrap().state.scroll,
            before + 3
        );
    }
}

/// 【终端】【普通正文回归】指针位于执行链外时直接滚动主会话。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_plain_body_scrolls_conversation_directly() {
    let mut runtime = runtime();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    let before = state.scroll;
    let row = state.layout.unwrap().body_top;
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 8, row))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before - 3
    );
}

/// 【终端】【思考边界回归】思考窗口直接滚动，末尾先阻挡再交给外层，反向回到内部。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_reasoning_uses_the_same_boundary_handoff() {
    let mut runtime = runtime();
    let state = &mut runtime.fullscreen.as_mut().unwrap().state;
    let layout = state.layout.unwrap();
    let mut store = TranscriptStore::new(1000);
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..21).map(|row| format!("thought-{row}\n")).collect(),
    });
    store.finalize_live_tail();
    let reasoning = store.render_fullscreen(
        usize::from(layout.content_width),
        &TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        },
        &Default::default(),
    );
    let mut document = source(20, 0);
    document.activities.clear();
    document.paragraphs.clear();
    document.lines.truncate(20);
    document.lines.extend(reasoning.lines);
    document
        .lines
        .extend((0..40).map(|row| AnsiLine::new(format!("ANSWER-{row}"))));
    document.reasonings = reasoning.reasonings;
    for region in &mut document.reasonings {
        region.start += 20;
        region.end += 20;
    }
    let document = state
        .reasoning
        .update(document, usize::from(layout.content_width));
    let document = state.activity.update(
        document,
        usize::from(layout.body_height),
        usize::from(layout.content_width),
        usize::from(layout.rows),
    );
    state.apply_document(document, usize::from(layout.body_height));
    let before = state.scroll;
    let row = layout.body_top + 3;
    for _ in 0..7 {
        runtime
            .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
            .unwrap();
        assert_eq!(runtime.fullscreen.as_ref().unwrap().state.scroll, before);
    }
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollDown, 8, row))
        .unwrap();
    assert_eq!(
        runtime.fullscreen.as_ref().unwrap().state.scroll,
        before + 3
    );
    let detail = runtime.fullscreen.as_ref().unwrap().state.document.lines[21].clone();
    runtime
        .handle_fullscreen_event(&mouse(MouseEventKind::ScrollUp, 8, row))
        .unwrap();
    let state = &runtime.fullscreen.as_ref().unwrap().state;
    assert_eq!(state.scroll, before + 3);
    assert_ne!(state.document.lines[21], detail);
}

/// 【终端】【详情高度回归】底部点击展开后，后续重绘仍使用正常详情高度预算。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_bottom_expansion_does_not_cache_tiny_height() {
    let mut state = FullscreenState::new();
    state.layout = Some(super::layout::FullscreenLayout::compute(84, 40, 5));
    let mut closed = source(30, 0);
    closed.activities[0].scrollable = false;
    closed.paragraphs[1].expanded = false;
    let document = state.activity.update(closed, 34, 80, 40);
    state.apply_document(document, 34);
    state.scroll_to(0, 34);
    state.toggle_at(31).unwrap();
    for _ in 0..3 {
        let document = state.activity.update(source(30, 100), 34, 80, 40);
        assert_eq!(
            document.activities[0].end - document.activities[0].start,
            34
        );
        state.apply_document(document, 34);
    }
}

/// 【终端】【正文收起回归】展开详情中的普通正文行支持单击收起当前步骤。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_body_click_collapses_the_step() {
    let mut state = FullscreenState::new();
    let document = state.activity.update(source(0, 100), 34, 80, 40);
    state.apply_document(document, 34);
    assert!(state.toggle_at(3).is_some());
    assert!(state.expanded.contains(&STEP));
}

/// 【终端】【步骤锚定回归】展开操作以所点击步骤为锚点，后续流式增长保留屏幕位置。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_expansion_anchors_the_operated_step() {
    let mut state = FullscreenState::new();
    let mut closed = source(20, 0);
    closed.activities[0].scrollable = false;
    closed.paragraphs[1].expanded = false;
    let document = state.activity.update(closed, 34, 80, 40);
    state.apply_document(document, 34);
    state.scroll_to(18, 34);
    let (key, offset) = state.toggle_at(21).unwrap();
    assert_eq!(offset, 3, "锚点应为操作步骤，不能使用分组标题");
    for count in [100, 120, 140] {
        let document = state.activity.update(source(20, count), 34, 80, 40);
        state.apply_document(document, 34);
        state.restore_toggle_anchor(key, offset, 34);
        let step = state
            .document
            .paragraphs
            .iter()
            .find(|span| span.key == STEP)
            .unwrap();
        assert_eq!(step.control - state.scroll, 3);
        assert!(state.document.lines[step.control + 1]
            .as_str()
            .contains("DETAIL-0"));
    }
}

/// 【终端】【状态占位回归】运行到完成不插拔页脚，短完成摘要仍保留单行状态。
/// 参数: 无；返回: 无。
#[test]
fn activity_interaction_footer_keeps_one_row_across_completion() {
    let mut views = super::activity_viewport::ActivityViewports::default();
    let mut running = source(0, 100);
    running.activities[0].active = true;
    let running = views.update(running, 34, 80, 40);
    let completed = views.update(source(0, 100), 34, 80, 40);
    assert_eq!(running.activities[0].end, completed.activities[0].end);
    assert_eq!(running.activities[0].footer, completed.activities[0].footer);
    let compact = views.update(source(0, 2), 34, 80, 40);
    let region = &compact.activities[0];
    assert_eq!(region.footer, Some(region.end - 1));
    assert!(!compact.lines[region.end - 1].as_str().contains('\n'));
}

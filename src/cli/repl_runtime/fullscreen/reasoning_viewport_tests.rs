use super::*;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【思考测试】参数为流式状态；返回真实渲染路径生成的长思考文档。
fn source(live: bool) -> FullscreenDocument {
    let mut store = TranscriptStore::new(1000);
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..20).map(|i| format!("thought-{i:02}\n")).collect(),
    });
    if !live {
        store.finalize_live_tail();
    }
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let document = store.render_fullscreen(80, &options, &Default::default());
    crate::render::tui_mode::set_override(None);
    document
}

/// 【终端】【思考回归】历史仅显示五行竖线正文，滚动不改变高度或原始数据；无参数和返回值。
#[test]
fn history_is_five_rows_and_scrolls_without_resizing() {
    let raw = source(false);
    assert_eq!(raw.reasonings.len(), 1);
    let key = raw.reasonings[0].key;
    let mut viewport = ReasoningViewports::default();
    let first = viewport.update(raw.clone(), 80);
    assert_eq!(first.lines.len(), 6);
    assert!(first.lines[1].as_str().contains("thought-00"));
    assert!(first
        .lines
        .iter()
        .skip(1)
        .all(|line| line.as_str().contains('│')));
    assert!(!viewport.scroll(key, -3));
    assert!(viewport.scroll(key, 3));
    let next = viewport.project();
    assert_eq!(next.lines.len(), first.lines.len());
    assert!(next.lines[1].as_str().contains("thought-03"));
    assert!(viewport.scroll(key, isize::MAX));
    assert!(!viewport.scroll(key, 3));
    assert!(viewport.project().lines[5].as_str().contains("thought-19"));
    assert_eq!(viewport.source.lines, raw.lines);
}

/// 【终端】【实时思考回归】默认跟随尾部，向上阅读后重绘不跳回末尾；无参数和返回值。
#[test]
fn live_reasoning_follows_until_user_scrolls_up() {
    let mut viewport = ReasoningViewports::default();
    let raw = source(true);
    let key = raw.reasonings[0].key;
    let first = viewport.update(raw.clone(), 80);
    assert!(first.lines[5].as_str().contains("thought-19"));
    assert!(viewport.scroll(key, -3));
    let next = viewport.update(raw, 80);
    assert!(next.lines[1].as_str().contains("thought-12"));
    assert_eq!(next.lines.len(), 6);
}

/// 【终端】【思考结算回归】实时自动跟随转为历史时从开头阅读，不依赖首帧到达时序。
/// 参数: 无；返回: 无，覆盖同一正文键跨实时和历史阶段的投影
#[test]
fn automatically_followed_live_reasoning_starts_history_at_the_top() {
    let live = source(true);
    let history = source(false);
    assert!(live.reasonings[0].live);
    assert!(!history.reasonings[0].live);
    assert_eq!(live.reasonings[0].key, history.reasonings[0].key);
    let mut viewport = ReasoningViewports::default();
    assert!(viewport.update(live, 80).lines[5]
        .as_str()
        .contains("thought-19"));
    let completed = viewport.update(history, 80);
    assert!(completed.lines[1].as_str().contains("thought-00"));
    assert_eq!(completed.lines.len(), 6);
}

/// 【终端】【阅读位置回归】用户已经滚动的实时正文转为历史后仍保留阅读位置。
/// 参数: 无；返回: 无，防止结算把主动阅读位置跳回开头
#[test]
fn manually_scrolled_live_reasoning_keeps_its_history_position() {
    let live = source(true);
    let key = live.reasonings[0].key;
    let mut viewport = ReasoningViewports::default();
    viewport.update(live, 80);
    assert!(viewport.scroll(key, -3));
    let history = viewport.update(source(false), 80);
    assert!(history.lines[1].as_str().contains("thought-12"));
}

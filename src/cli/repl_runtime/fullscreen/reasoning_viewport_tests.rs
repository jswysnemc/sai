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
    crate::render::tui_style::set_override(Some(crate::config::TuiStyle::Deepseek));
    let document = store.render_fullscreen(80, &options, &Default::default());
    crate::render::tui_style::set_override(None);
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

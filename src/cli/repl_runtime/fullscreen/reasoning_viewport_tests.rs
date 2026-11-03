use super::*;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【思考测试】参数为流式状态；返回真实渲染路径生成的长思考文档。
fn source(live: bool) -> FullscreenDocument {
    source_with_dimensions(live, 20, 80)
}

/// 【终端】【思考测试】按指定行数和终端宽度生成真实思考文档。
/// 参数: `live` 为流式状态，`rows` 为正文行数，`width` 为终端列数；返回: 完整源文档。
fn source_with_dimensions(live: bool, rows: usize, width: usize) -> FullscreenDocument {
    let mut store = TranscriptStore::new(1000);
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..rows).map(|i| format!("thought-{i:02}\n")).collect(),
    });
    if !live {
        store.finalize_live_tail();
    }
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let document = store.render_fullscreen(width, &options, &Default::default());
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

/// 【终端】【滑块测试】读取窗口正文中青色滑块所在的行号。
/// 参数: `document` 为单段思考投影；返回: 相对正文首行的滑块行号。
fn thumb_rows(document: &FullscreenDocument) -> Vec<usize> {
    let palette = super::super::sidebar_palette::SidebarPalette::current();
    let accent = format!("{}│", super::super::sidebar_palette::fg(palette.accent));
    let track = format!("{}│", super::super::sidebar_palette::fg(palette.muted));
    document
        .lines
        .iter()
        .skip(1)
        .enumerate()
        .filter_map(|(row, line)| {
            assert!(
                line.as_str().contains(&accent) || line.as_str().contains(&track),
                "正文第 {row} 行缺少滚动条颜色: {:?}",
                line.as_str()
            );
            line.as_str().contains(&accent).then_some(row)
        })
        .collect()
}

/// 【终端】【双色滚动回归】从开头滚动到中间和末尾时滑块同步移动，源文档保持不变。
/// 参数: 无；返回: 无。
#[test]
fn scrollbar_moves_with_the_reasoning_window() {
    let raw = source(false);
    let key = raw.reasonings[0].key;
    let mut viewport = ReasoningViewports::default();
    let first = viewport.update(raw.clone(), 80);
    assert_eq!(thumb_rows(&first), vec![0]);
    assert!(viewport.scroll(key, 8));
    assert_eq!(thumb_rows(&viewport.project()), vec![2]);
    assert!(viewport.scroll(key, isize::MAX));
    assert_eq!(thumb_rows(&viewport.project()), vec![4]);
    assert_eq!(viewport.source.lines, raw.lines);
}

/// 【终端】【实时占比回归】流式正文增长时滑块缩短并跟随末尾，结算后回到开头。
/// 参数: 无；返回: 无。
#[test]
fn scrollbar_resizes_as_live_reasoning_grows() {
    let mut viewport = ReasoningViewports::default();
    assert_eq!(
        thumb_rows(&viewport.update(source_with_dimensions(true, 10, 80), 80)),
        vec![3, 4]
    );
    let longer = viewport.update(source(true), 80);
    assert_eq!(thumb_rows(&longer), vec![4]);
    assert!(longer.lines[5].as_str().contains("thought-19"));
    assert_eq!(thumb_rows(&viewport.update(source(false), 80)), vec![0]);
}

/// 【终端】【短段占比回归】整段正文可见时引导线全部使用滑块颜色，不添加滚动提示。
/// 参数: 无；返回: 无。
#[test]
fn short_reasoning_uses_a_full_length_thumb() {
    let mut viewport = ReasoningViewports::default();
    let document = viewport.update(source_with_dimensions(false, 3, 80), 80);
    assert_eq!(document.lines.len(), 4);
    assert_eq!(thumb_rows(&document), vec![0, 1, 2]);
    assert!(!document.lines[0].as_str().contains('↕'));
}

/// 【终端】【窄窗口回归】标题无法容纳行数提示时仍显示双色引导线，正文行宽保持不变。
/// 参数: 无；返回: 无。
#[test]
fn narrow_reasoning_keeps_the_scrollbar_without_a_title_hint() {
    let mut viewport = ReasoningViewports::default();
    let document = viewport.update(source_with_dimensions(false, 20, 18), 18);
    assert_eq!(thumb_rows(&document), vec![0]);
    assert!(!document.lines[0].as_str().contains('↕'));
    assert!(document
        .lines
        .iter()
        .all(|line| crate::render::ansi_clip::ansi_visible_width(line.as_str()) <= 18));
}

use super::super::test_support::{chunk, options};
use super::*;
use crate::config::TuiStyle;
use crate::render::{activity_animation::strip_ansi_for_test, tui_style};

/// 【终端】【详情窗口测试】追加已完成的文本读取，用正文行数控制窗口占用。
/// 参数: `store` 为记录，`path` 为路径，`lines` 为正文行数；返回: 无。
fn read(store: &mut TranscriptStore, path: &str, lines: usize) {
    store.push_tool_call(
        "read_file".into(),
        serde_json::json!({"path":path}).to_string(),
    );
    store.push_tool_result(
        "read_file".into(),
        true,
        (0..lines)
            .map(|index| format!("{}\t{path}-DETAIL-{index}", index + 1))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// 【终端】【详情窗口测试】读取默认展开状态下的完整源文档纯文本。
/// 参数: `store` 为记录；返回: 去除 ANSI 的正文。
fn text(store: &mut TranscriptStore) -> String {
    store
        .render_fullscreen(100, &options(), &HashSet::new())
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【终端】【近期详情测试】新工具出现不立即折叠之前的短思考和短工具，工具完成后保持展开。
/// 参数: 无；返回: 无。
#[test]
fn recent_details_stay_open_until_new_steps_push_them_out() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("EARLY-THOUGHT".into()));
    read(&mut store, "first.txt", 1);
    let recent = text(&mut store);
    assert!(recent.contains("EARLY-THOUGHT"), "{recent}");
    assert!(recent.contains("first.txt-DETAIL-0"), "{recent}");
    read(&mut store, "second.txt", 1);
    assert!(text(&mut store).contains("first.txt-DETAIL-0"));
    read(&mut store, "large.txt", 10);
    let advanced = text(&mut store);
    assert!(!advanced.contains("EARLY-THOUGHT"), "{advanced}");
    assert!(!advanced.contains("first.txt-DETAIL-0"), "{advanced}");
    assert!(advanced.contains("large.txt-DETAIL-9"), "{advanced}");
    assert!(advanced.contains("Thought"), "{advanced}");
    tui_style::set_override(None);
}

/// 【终端】【思考窗口测试】新的长思考也会把旧工具详情推出十行窗口，而不是只有工具才参与。
/// 参数: 无；返回: 无。
#[test]
fn new_reasoning_can_push_old_tool_details_out_of_the_window() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    read(&mut store, "first.txt", 1);
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "CURRENT-THOUGHT\n"));
    assert!(text(&mut store).contains("first.txt-DETAIL-0"));
    store.push_chunk(&chunk(
        ChatStreamKind::Reasoning,
        &"Long thought line\n".repeat(10),
    ));
    assert!(!text(&mut store).contains("first.txt-DETAIL-0"));
    store.finalize_live_tail();
    assert!(text(&mut store).contains("CURRENT-THOUGHT"));
    tui_style::set_override(None);
}

/// 【终端】【手动折叠测试】近期工具可以手动收起，窗口外的历史工具仍可以手动展开。
/// 参数: 无；返回: 无。
#[test]
fn recent_detail_window_respects_manual_fold_and_expand() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    read(&mut store, "first.txt", 1);
    read(&mut store, "second.txt", 1);
    let document = store.render_fullscreen(100, &options(), &HashSet::new());
    let span = document
        .paragraphs
        .iter()
        .find(|span| {
            span.key.cell == 0
                && span.key.part != super::super::fullscreen_parts::ParagraphPart::ActivityGroup
        })
        .unwrap();
    assert!(span.expanded);
    let folded = store.render_fullscreen(100, &options(), &HashSet::from([span.key]));
    assert!(!folded
        .lines
        .iter()
        .any(|line| line.as_str().contains("first.txt-DETAIL-0")));
    read(&mut store, "large.txt", 10);
    let opened = store.render_fullscreen(100, &options(), &HashSet::from([ParagraphKey::whole(0)]));
    assert!(opened
        .lines
        .iter()
        .any(|line| line.as_str().contains("first.txt-DETAIL-0")));
    tui_style::set_override(None);
}

/// 【终端】【窗口边界测试】较新内容占九行时保留旧详情，达到十行时才折叠。
/// 参数: 无；返回: 无。
#[test]
fn exactly_ten_newer_rows_fold_the_previous_detail() {
    for newer in [9, 10] {
        let mut details = recent_details::RecentDetails::default();
        let mut rows = newer;
        let (key, node) = details.render(0, &mut rows, false, &HashSet::new(), |open| {
            node::RenderedNode {
                lines: vec![AnsiLine::new("old".into()); if open { 3 } else { 1 }],
                open,
            }
        });
        assert_eq!(node.open, newer == 9);
        assert_eq!(key.is_active(), newer == 9);
    }
}

/// 【终端】【折叠稳定测试】较新节点定稿变短后，已经退出窗口的旧详情不自动重开。
/// 参数: 无；返回: 无。
#[test]
fn folded_details_do_not_reopen_when_newer_content_shrinks() {
    let mut details = recent_details::RecentDetails::default();
    for newer in [10, 1] {
        let mut rows = newer;
        let (_, node) = details.render(0, &mut rows, false, &HashSet::new(), |open| {
            node::RenderedNode {
                lines: vec![AnsiLine::new("old".into())],
                open,
            }
        });
        assert!(!node.open);
    }
}

/// 【终端】【状态延续测试】思考从流式转为定稿时沿用手动收起键，不闪现完整正文。
/// 参数: 无；返回: 无。
#[test]
fn finalized_reasoning_keeps_its_manual_fold_key() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    read(&mut store, "first.txt", 1);
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "STABLE-THOUGHT"));
    let live = store.render_fullscreen(100, &options(), &HashSet::new());
    let key = live
        .paragraphs
        .iter()
        .find(|span| span.key.cell == 1)
        .unwrap()
        .key;
    let folded = HashSet::from([key]);
    assert!(!store
        .render_fullscreen(100, &options(), &folded)
        .lines
        .iter()
        .any(|line| line.as_str().contains("STABLE-THOUGHT")));
    store.finalize_live_tail();
    let finalized = store.render_fullscreen(100, &options(), &folded);
    assert!(finalized
        .paragraphs
        .iter()
        .any(|span| span.key == key && !span.expanded));
    assert!(!finalized
        .lines
        .iter()
        .any(|line| line.as_str().contains("STABLE-THOUGHT")));
    tui_style::set_override(None);
}

/// 【终端】【窗口重置测试】清空会话时清除自动折叠状态，复用下标不会收起新工具。
/// 参数: 无；返回: 无。
#[test]
fn clearing_transcript_resets_recent_details() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    read(&mut store, "old.txt", 1);
    read(&mut store, "large.txt", 10);
    assert!(!text(&mut store).contains("old.txt-DETAIL-0"));
    store.clear();
    read(&mut store, "fresh.txt", 1);
    read(&mut store, "second.txt", 1);
    assert!(text(&mut store).contains("fresh.txt-DETAIL-0"));
    tui_style::set_override(None);
}

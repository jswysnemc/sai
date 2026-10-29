use super::*;
use crate::config::TuiMode;
use crate::render::{ansi_clip::strip_ansi, tui_mode};
use std::time::{Duration, Instant};

/// 【终端】【恢复排版测试】通过完整全屏入口读取历史节点，而非直接调用折行辅助函数。
/// 参数: `store` 为历史记录，`width` 为终端列宽；返回: 去除控制序列的显示行。
fn visible_lines(store: &mut TranscriptStore, width: usize) -> Vec<String> {
    store
        .render_fullscreen(
            width,
            &super::super::test_support::options(),
            &HashSet::new(),
        )
        .lines
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .collect()
}

/// 【终端】【读取恢复测试】读取代码的续行必须对齐首行代码，而不是行号或固定间隔。
/// 参数: 无；返回: 无。
#[test]
fn restored_read_aligns_wrapped_code_including_source_indent() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_history_tool_call("read_file".into(), r#"{"path":"fixture.rs"}"#.into());
    store.push_tool_result(
        "read_file".into(),
        true,
        format!("120\t        {}", "x".repeat(90)),
    );
    let lines = visible_lines(&mut store, 40);
    let first = lines
        .iter()
        .position(|line| line.trim_start().starts_with("120 ") && line.contains('x'))
        .unwrap();
    let column = lines[first].find('x').unwrap();
    assert_eq!(lines[first + 1].find('x'), Some(column), "{lines:#?}");
    tui_mode::set_override(None);
}

/// 【终端】【补丁恢复测试】补丁上下文与增删行的续行均保留原代码缩进。
/// 参数: 无；返回: 无。
#[test]
fn restored_diff_aligns_wrapped_code_including_source_indent() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_history_tool_call(
        "str_replace".into(),
        r#"{"path":"fixture.rs","old_string":"old","new_string":"new"}"#.into(),
    );
    let diff = format!(
        "--- a/fixture.rs\n+++ b/fixture.rs\n@@ -120 +120 @@\n-        {}\n+        {}\n",
        "x".repeat(90),
        "y".repeat(90)
    );
    store.push_tool_result("str_replace".into(), true, serde_json::json!({"ok":true,"diff":diff,"changed_files":[{"path":"fixture.rs","added":1,"removed":1}]}).to_string());
    let lines = visible_lines(&mut store, 40);
    for marker in ['x', 'y'] {
        let first = lines
            .iter()
            .position(|line| line.contains("120") && line.contains(marker))
            .unwrap();
        let column = lines[first].find(marker).unwrap();
        assert_eq!(lines[first + 1].find(marker), Some(column), "{lines:#?}");
    }
    tui_mode::set_override(None);
}

/// 【终端】【思考动效测试】实际普通思考分支必须响应时钟，正文和定稿节点保持稳定。
/// 参数: 无；返回: 无。
#[test]
fn live_thinking_pulses_through_the_fullscreen_entrypoint() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_chunk(&super::super::test_support::chunk(
        ChatStreamKind::Reasoning,
        "Stable reasoning body",
    ));
    store.work_status_started = Some(Instant::now());
    let first = visible_lines(&mut store, 80);
    store.work_status_started = Some(Instant::now() - Duration::from_millis(320));
    let next = visible_lines(&mut store, 80);
    assert!(first[0].starts_with('◇'), "{first:?}");
    assert!(next[0].starts_with('◆'), "{next:?}");
    assert_eq!(&first[1..], &next[1..]);
    store.finalize_live_tail();
    let done = visible_lines(&mut store, 80);
    store.work_status_started = Some(Instant::now() - Duration::from_millis(480));
    assert_eq!(done, visible_lines(&mut store, 80));
    tui_mode::set_override(None);
}

use super::cell::{HistoryCell, TranscriptMode};
use super::test_support::options;
use super::TranscriptStore;
use crate::render::activity_animation::strip_ansi_for_test;

/// 【终端】【布局层次】混合 cell 应使用不同引导符，并在正文/思考/提示前留空行。
#[test]
fn mixed_cells_use_distinct_markers_and_section_gaps() {
    let opts = options();
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Automatic, "ask".into());
    store.push_cell(HistoryCell::reasoning("consider options".into()));
    store.push_tool_call("read_file".into(), r#"{"path":"a.rs"}"#.into());
    store.push_cell(HistoryCell::markdown("Here is the answer.".into()));
    store.push_meta("model switched".into());

    let mut snapshot = Vec::new();
    for cell in &store.cells {
        let lines = cell.display_lines(80, &opts);
        let plain: Vec<String> = lines
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect();
        snapshot.push(plain);
    }

    // user: 轮次前空行 + ●
    assert!(snapshot[0][0].is_empty());
    assert!(snapshot[0][1].starts_with('●'));

    // reasoning: 仅前空行 + ◦（不再 trailing，避免与后续 Markdown 前空行叠成两行）
    assert!(snapshot[1][0].is_empty());
    assert!(
        snapshot[1][1].starts_with('◦'),
        "thinking marker: {:?}",
        snapshot[1][1]
    );
    assert!(!snapshot[1][1].starts_with('•'));
    assert!(
        snapshot[1].last().is_some_and(|line| !line.is_empty()),
        "thinking must not trail a blank: {:?}",
        snapshot[1]
    );

    // tool: 前空行 + • 标记
    assert!(snapshot[2][0].is_empty());
    assert!(
        snapshot[2][1].starts_with('•'),
        "tool marker: {:?}",
        snapshot[2][1]
    );

    // markdown: 区块空行 + 无 • 的缩进正文
    assert!(snapshot[3][0].is_empty());
    assert!(
        snapshot[3][1].starts_with("  Here"),
        "body should be indent-only: {:?}",
        snapshot[3][1]
    );
    assert!(!snapshot[3][1].contains('•'));

    // meta: 区块空行 + ›
    assert!(snapshot[4][0].is_empty());
    assert!(
        snapshot[4][1].starts_with('›'),
        "meta marker: {:?}",
        snapshot[4][1]
    );

}

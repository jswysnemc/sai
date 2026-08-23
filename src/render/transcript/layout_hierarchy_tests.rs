use super::cell::{HistoryCell, TranscriptMode};
use super::test_support::{chunk, options};
use super::TranscriptStore;
use crate::llm::ChatStreamKind;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::work_status::WorkStatus;

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

    // tool: 自身无前空行，紧跟思考标题/正文
    assert!(
        snapshot[2][0].starts_with('•'),
        "tool marker: {:?}",
        snapshot[2][0]
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

/// 【终端】【块间距】正文与后续工具之间必须空一行；思考后的工具仍紧挨。
#[test]
fn assembled_window_separates_body_from_following_tool() {
    let opts = options();
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Automatic, "ask".into());
    store.push_cell(HistoryCell::reasoning("consider options".into()));
    store.push_tool_call("read_file".into(), r#"{"path":"a.rs"}"#.into());
    store.push_cell(HistoryCell::markdown("Here is the answer.".into()));
    store.push_tool_call("read_file".into(), r#"{"path":"b.rs"}"#.into());

    let plain: Vec<String> = store
        .display_tail(80, &opts)
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect();
    let think = plain.iter().position(|line| line.starts_with('◦'));
    let first_tool = plain.iter().position(|line| line.starts_with('•'));
    let body = plain.iter().position(|line| line.contains("Here is"));
    let second_tool = plain
        .iter()
        .enumerate()
        .rev()
        .find(|(_, line)| line.starts_with('•'))
        .map(|(index, _)| index);

    assert!(think < first_tool, "{plain:?}");
    assert!(
        first_tool
            .and_then(|index| index.checked_sub(1))
            .is_some_and(|prev| !plain[prev].trim().is_empty()),
        "thinking must sit against the first tool: {plain:?}"
    );
    assert!(body < second_tool, "{plain:?}");
    assert!(
        second_tool
            .and_then(|index| index.checked_sub(1))
            .is_some_and(|prev| plain[prev].trim().is_empty()),
        "body must be separated from the next tool: {plain:?}"
    );
}

/// 【终端】【块间距】流式正文与工具预览同时在 live 区时，中间必须空一行。
#[test]
fn live_content_and_tool_preview_keep_section_gap() {
    let opts = options();
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Automatic, "ask".into());
    store.push_chunk(&chunk(ChatStreamKind::Content, "Here is the answer.\n"));
    store.push_tool_call_progress(&crate::llm::ToolCallStreamProgress {
        index: 0,
        name: Some("read_file".into()),
        arguments_chars: 16,
        arguments_bytes: 16,
        arguments_preview: r#"{"path":"a.rs"}"#.into(),
    });

    let lines = plain_tail(&mut store, &opts);
    assert_no_consecutive_blanks(&lines);
    assert_body_preceded_by_blank(&lines);
    assert_preceded_by_blank(&lines, '•', true);
}

/// 【终端】【块间距】正文已定稿、工具仍在 live 预览时，中间也必须空一行。
#[test]
fn finalized_body_and_live_tool_preview_keep_section_gap() {
    let opts = options();
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Automatic, "ask".into());
    store.push_chunk(&chunk(ChatStreamKind::Content, "Here is the answer."));
    store.finalize_live_tail();
    store.push_tool_call_progress(&crate::llm::ToolCallStreamProgress {
        index: 0,
        name: Some("read_file".into()),
        arguments_chars: 16,
        arguments_bytes: 16,
        arguments_preview: r#"{"path":"a.rs"}"#.into(),
    });

    let lines = plain_tail(&mut store, &opts);
    assert_no_consecutive_blanks(&lines);
    assert_preceded_by_blank(&lines, '•', true);
}

/// 【终端】【块间距】流式中间态与定稿后的块间空行必须一致，且不能出现连续空行。
#[test]
fn streaming_section_gaps_match_finalized_cells() {
    let opts = options();
    let mut live = TranscriptStore::new(100);
    live.push_user_echo(TranscriptMode::Automatic, "ask".into());
    live.set_work_status(WorkStatus::Thinking);
    live.push_chunk(&chunk(ChatStreamKind::Reasoning, "consider options\n"));

    let mid_reasoning = plain_tail(&mut live, &opts);
    assert_no_consecutive_blanks(&mid_reasoning);
    assert_preceded_by_blank(&mid_reasoning, '◦', true);

    live.finalize_live_tail();
    live.push_tool_call("read_file".into(), r#"{"path":"a.rs"}"#.into());
    live.set_work_status(WorkStatus::Working);
    let mid_tool = plain_tail(&mut live, &opts);
    assert_no_consecutive_blanks(&mid_tool);
    assert_preceded_by_blank(&mid_tool, '•', false);

    live.push_tool_result("read_file".into(), true, "ok".into());
    live.push_chunk(&chunk(ChatStreamKind::Content, "Here is the answer.\n"));
    let mid_content = plain_tail(&mut live, &opts);
    assert_no_consecutive_blanks(&mid_content);
    assert_body_preceded_by_blank(&mid_content);

    live.finalize_live_tail();
    live.clear_work_status();
    let finalized = plain_tail(&mut live, &opts);

    let mut expected = TranscriptStore::new(100);
    expected.push_user_echo(TranscriptMode::Automatic, "ask".into());
    expected.push_cell(HistoryCell::reasoning("consider options\n".into()));
    expected.push_tool_call("read_file".into(), r#"{"path":"a.rs"}"#.into());
    expected.push_tool_result("read_file".into(), true, "ok".into());
    expected.push_cell(HistoryCell::markdown("Here is the answer.\n".into()));
    let expected_lines = plain_tail(&mut expected, &opts);

    assert_eq!(
        gap_signature(&finalized),
        gap_signature(&expected_lines),
        "finalized={finalized:?}\nexpected={expected_lines:?}"
    );
    assert_no_consecutive_blanks(&finalized);
}

/// 【终端】【块间距】思考还在 live 时工具预览跟上，中间不能多出空行。
#[test]
fn live_reasoning_and_tool_preview_share_single_gap_rules() {
    let opts = options();
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Automatic, "ask".into());
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "consider options\n"));
    store.push_tool_call_progress(&crate::llm::ToolCallStreamProgress {
        index: 0,
        name: Some("read_file".into()),
        arguments_chars: 16,
        arguments_bytes: 16,
        arguments_preview: r#"{"path":"a.rs"}"#.into(),
    });

    let lines = plain_tail(&mut store, &opts);
    assert_no_consecutive_blanks(&lines);
    assert_preceded_by_blank(&lines, '◦', true);
    assert_preceded_by_blank(&lines, '•', false);
}

/// 渲染当前窗口为去 ANSI 后的纯文本行。
fn plain_tail(store: &mut TranscriptStore, opts: &super::TranscriptRenderOptions) -> Vec<String> {
    store
        .display_tail(80, opts)
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect()
}

/// 断言窗口里没有连续两行视觉空行。
fn assert_no_consecutive_blanks(lines: &[String]) {
    for pair in lines.windows(2) {
        assert!(
            !(pair[0].trim().is_empty() && pair[1].trim().is_empty()),
            "consecutive blanks: {lines:?}"
        );
    }
}

/// 断言指定引导符那一行前面是否恰好有一块空行。
fn assert_preceded_by_blank(lines: &[String], marker: char, expect_blank: bool) {
    let index = lines
        .iter()
        .position(|line| line.trim_start().starts_with(marker))
        .unwrap_or_else(|| panic!("missing marker {marker}: {lines:?}"));
    let previous_blank = index
        .checked_sub(1)
        .is_some_and(|prev| lines[prev].trim().is_empty());
    assert_eq!(
        previous_blank, expect_blank,
        "marker {marker} gap mismatch: {lines:?}"
    );
}

/// 断言正文缩进行前面有一块空行。
fn assert_body_preceded_by_blank(lines: &[String]) {
    let index = lines
        .iter()
        .position(|line| line.starts_with("  Here"))
        .unwrap_or_else(|| panic!("missing body: {lines:?}"));
    assert!(
        index
            .checked_sub(1)
            .is_some_and(|prev| lines[prev].trim().is_empty()),
        "body should follow a section blank: {lines:?}"
    );
}

/// 把每一行压缩成「是否视觉空行」，用于比较流式定稿与直接 push 的间隔。
fn gap_signature(lines: &[String]) -> Vec<bool> {
    lines.iter().map(|line| line.trim().is_empty()).collect()
}

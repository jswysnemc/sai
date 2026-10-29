use super::super::{spacing, test_support::options};
use super::*;
use crate::config::TuiMode;
use crate::render::{
    render_expand::with_force_expand, tui_mode, ReasoningDisplayMode, ToolCallDisplayMode,
};

/// 【终端】【问答回归】执行链直接展示问题与答案，不展开协议字段；无参数和返回值。
#[test]
fn question_timeline_uses_compact_semantic_details() {
    let mut view = crate::render::tool_view::ToolView::running(
        "ask_question".into(),
        serde_json::json!({"questions":[{
            "header":"报告内容选择", "question":"报告包含哪些内容？", "multiple":true,
            "options":[{"label":"调用矩阵","description":"重复说明","value":"matrix"},
                       {"label":"异常用例","description":"另一说明"}]
        }]})
        .to_string(),
    );
    view.finish(
        true,
        serde_json::json!({"status":"answered","answers":[{
        "question":"报告包含哪些内容？","answer":["matrix"]
    }],"instruction":"INTERNAL_INSTRUCTION"})
        .to_string(),
    );
    let cell = HistoryCell::Tool(super::super::tool_cell::ToolCell::Invocation(Box::new(
        view,
    )));
    let node = node::render_cell(&cell, 80, false, true);
    let plain = crate::render::activity_animation::strip_ansi_for_test(
        &node
            .lines
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );
    assert!(plain.contains("报告包含哪些内容？"), "{plain}");
    assert_eq!(plain.matches("调用矩阵").count(), 1, "{plain}");
    assert!(plain.contains("●") && !plain.contains("○"), "{plain}");
    assert!(!plain.contains("异常用例"), "{plain}");
    for redundant in [
        "Header:",
        "Multiple:",
        "Description:",
        "Label:",
        "Input:",
        "Output:",
        "INTERNAL_INSTRUCTION",
        "重复说明",
        "□",
        "■",
    ] {
        assert!(!plain.contains(redundant), "{redundant}: {plain}");
    }
}

/// 【终端】【普通节点测试】渲染普通样式的完整单元，去掉布局边界空行。
/// 参数: `cell` 为历史单元，`width` 为净宽；返回: 普通样式的完整物理行。
fn standard_lines(cell: &HistoryCell, width: usize) -> Vec<AnsiLine> {
    let full = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    let mut lines = with_force_expand(|| cell.display_lines_framed(width, &full, 0));
    spacing::trim_leading_visual_blanks(&mut lines);
    spacing::trim_trailing_visual_blanks(&mut lines);
    lines
}

/// 【终端】【命令样式测试】展开命令直接复用普通样式，不额外渲染 JSON 或第二个标题。
/// 参数: 无；返回: 无。
#[test]
fn expanded_command_reuses_standard_rendering() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_tool_call(
        "run_command".into(),
        serde_json::json!({"command":format!("cat /tmp/{}", "long-path-".repeat(10))}).to_string(),
    );
    store.push_tool_result(
        "run_command".into(),
        true,
        r#"{"success":true,"exit_code":0,"stdout":"result-line\nnext-line","stderr":""}"#.into(),
    );
    let expected = standard_lines(&store.cells[0], 65);
    let actual = node::render_cell(&store.cells[0], 65, false, true);
    assert_eq!(actual.lines, expected);
    tui_mode::set_override(None);
}

/// 【终端】【思考样式测试】思考展开使用普通样式的标题、正文颜色和缩进。
/// 参数: 无；返回: 无。
#[test]
fn expanded_reasoning_reuses_standard_rendering() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let cell = HistoryCell::reasoning("**reasoning detail**\n继续检查布局与工具状态切换".into());
    assert_eq!(
        node::render_cell(&cell, 65, false, true).lines,
        standard_lines(&cell, 65)
    );
    tui_mode::set_override(None);
}

/// 【终端】【思考切换测试】流式思考定稿后保持正文颜色、行数和缩进，不重新换成 Markdown 展示。
/// 参数: 无；返回: 无。
#[test]
fn reasoning_transition_preserves_body_layout() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_chunk(&super::super::test_support::chunk(
        ChatStreamKind::Reasoning,
        "**reasoning detail**\n继续检查布局与工具状态切换",
    ));
    let live = node::render_live(&store, node::LiveNode::Thought, 65, true);
    store.finalize_live_tail();
    let done = node::render_cell(&store.cells[0], 65, false, true);
    assert_eq!(&live.lines[1..], &done.lines[1..]);
    let document = store.render_fullscreen(65, &options(), &HashSet::new());
    assert_eq!(document.lines.len(), done.lines.len());
    assert_eq!(document.paragraphs.len(), 1);
    assert_eq!(document.paragraphs[0].control, document.paragraphs[0].start);
    tui_mode::set_override(None);
}

/// 【终端】【命令准备测试】未闭合参数只显示已经收到的命令，标题与执行阶段使用相同正文列。
/// 参数: 无；返回: 无。
#[test]
fn preparing_command_reuses_standard_header_without_json() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    for preview in [
        "",
        r#"{"command":"printf 'ready'"#,
        r#"{"command":"printf 'ready'","timeout":30}"#,
    ] {
        store.live_tool_call = Some(super::super::store::LiveToolCall {
            name: "run_command".into(),
            arguments_preview: preview.into(),
            edit_diff_counts: None,
        });
        let node = node::render_live(&store, node::LiveNode::Tool, 65, true);
        let plain = node
            .lines
            .iter()
            .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("Preparing"), "{plain}");
        assert!(!plain.contains("\"command\""), "{plain}");
        assert!(!plain.contains("timeout"), "{plain}");
        if !preview.is_empty() {
            assert!(plain.contains("printf 'ready'"), "{plain}");
        }
    }
    tui_mode::set_override(None);
}

/// 【终端】【桌面链测试】桌面工具复用语义化普通样式，近期详情与手动收起均保留同一标题。
/// 参数: 无；返回: 无。
#[test]
fn computer_use_steps_reuse_semantic_rendering_inside_the_chain() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("Inspect the target window".into()));
    store.push_tool_call(
        "computer_screenshot".into(),
        r#"{"app":"42","window_id":101}"#.into(),
    );
    store.push_tool_result(
        "computer_screenshot".into(),
        true,
        "Screenshot: 100x60".into(),
    );
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let plain = document
        .lines
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(plain.matches("Captured window").count(), 1, "{plain}");
    assert!(plain.contains("Screenshot: 100x60"));
    assert!(!plain.contains("{\""));
    let collapsed = store.render_fullscreen(
        80,
        &options(),
        &HashSet::from([ParagraphKey::activity(1, false)]),
    );
    let collapsed_plain = collapsed
        .lines
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        collapsed_plain.contains("Captured window"),
        "{collapsed_plain}"
    );
    assert!(!collapsed
        .lines
        .iter()
        .any(|line| line.as_str().contains("Screenshot: 100x60")));
    tui_mode::set_override(None);
}

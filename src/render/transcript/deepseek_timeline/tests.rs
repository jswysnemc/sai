use super::super::test_support::{chunk, options};
use super::*;
use crate::config::TuiStyle;
use crate::llm::ChatStreamKind;
use crate::render::tui_style;

/// 【终端】【执行链测试】读取完整文档的纯文本，避免断言依赖配色。
/// 参数: `document` 为文档；返回: 不带控制序列的文本。
fn text(document: &FullscreenDocument) -> String {
    crate::render::activity_animation::strip_ansi_for_test(
        &document
            .lines
            .iter()
            .map(AnsiLine::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// 【终端】【执行链语言测试】中文设置下仍用英文计数和摘要，不改变原始思考与工具内容。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_uses_english_labels_under_chinese_locale() {
    crate::i18n::with_locale(crate::i18n::Locale::Zh, || {
        tui_style::set_override(Some(TuiStyle::Deepseek));
        let mut store = TranscriptStore::new(200);
        store.push_cell(HistoryCell::reasoning("原始思考".into()));
        store.push_tool_call("read_file".into(), r#"{"path":"fixture.rs"}"#.into());
        store.push_tool_result("read_file".into(), true, "1\t原始文件内容".into());
        let document = store.render_fullscreen(
            100,
            &options(),
            &HashSet::from([ParagraphKey::whole(0), ParagraphKey::whole(1)]),
        );
        let visible = text(&document);
        assert!(visible.contains("1 tool · 1 thought"), "{visible}");
        assert!(visible.contains("Thought"), "{visible}");
        assert!(visible.contains("Read file"), "{visible}");
        assert!(visible.contains("原始思考"), "{visible}");
        assert!(visible.contains("原始文件内容"), "{visible}");
        assert!(!visible.contains("已思考"));
        assert!(!visible.contains("读取文件"));
        assert_eq!(crate::i18n::locale(), crate::i18n::Locale::Zh);
        tui_style::set_override(None);
    });
}

/// 【终端】【执行链测试】手动收起的连续步骤紧凑排列，不保留竖线或详情。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_matches_linear_collapsed_structure() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let mut store = TranscriptStore::new(200);
        store.push_cell(HistoryCell::reasoning("OLD-THOUGHT".into()));
        store.push_tool_call(
            "read_file".into(),
            r#"{"path":"fixture.rs","offset":1,"limit":20}"#.into(),
        );
        store.push_tool_result("read_file".into(), true, "1\tOLD-OUTPUT\n2\tsecond".into());
        let document = store.render_fullscreen(
            100,
            &options(),
            &HashSet::from([
                ParagraphKey::activity(0, false),
                ParagraphKey::activity(1, false),
            ]),
        );
        let visible = text(&document);
        assert!(visible.contains("⌄ 1 tool · 1 thought"));
        assert!(!visible.contains('│'));
        assert_eq!(document.lines.len(), 3);
        assert!(visible.contains("Thought"));
        assert!(visible.contains("• Read"));
        assert!(visible.contains("fixture.rs (L1–2)"));
        assert!(!visible.contains("OLD-THOUGHT"));
        assert!(!visible.contains("OLD-OUTPUT"));
        assert!(!visible.contains("click to expand"));
        assert_eq!(document.paragraphs.len(), 3);
    });
    tui_style::set_override(None);
}

/// 【终端】【执行链测试】分组整体折叠保留子步骤展开键，重新打开后恢复选择。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_group_collapse_preserves_child_state() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("FULL-THOUGHT".into()));
    store.push_tool_call("read_file".into(), r#"{"path":"fixture.rs"}"#.into());
    store.push_tool_result("read_file".into(), true, "FULL-OUTPUT".into());
    let thought = ParagraphKey::whole(0);
    let tool = ParagraphKey::whole(1);
    let group = ParagraphKey::activity(0, true);
    let expanded = HashSet::from([thought, tool]);
    let open = store.render_fullscreen(100, &options(), &expanded);
    assert!(text(&open).contains("FULL-THOUGHT"));
    assert!(text(&open).contains("FULL-OUTPUT"));
    let closed = store.render_fullscreen(100, &options(), &HashSet::from([thought, tool, group]));
    assert!(!text(&closed).contains("FULL-THOUGHT"));
    assert!(!text(&closed).contains("FULL-OUTPUT"));
    assert!(closed.retained_keys.contains(&thought));
    assert!(closed.retained_keys.contains(&tool));
    assert_eq!(closed.paragraphs.len(), 1);
    assert!(!closed.paragraphs[0].expanded);
    tui_style::set_override(None);
}

/// 【终端】【执行链测试】近期思考和工具保持详情，工具完成或普通回答开始不触发折叠。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_recent_steps_stay_open_after_completion() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "CURRENT-THOUGHT"));
    assert!(
        text(&store.render_fullscreen(100, &options(), &HashSet::new()))
            .contains("CURRENT-THOUGHT")
    );
    store.push_tool_call("read_file".into(), r#"{"path":"CURRENT-FILE"}"#.into());
    store.push_tool_progress("read_file".into(), "CURRENT-PROGRESS".into());
    let running = text(&store.render_fullscreen(100, &options(), &HashSet::new()));
    assert!(running.contains("CURRENT-THOUGHT"));
    assert!(running.contains("CURRENT-PROGRESS"));
    store.push_tool_result("read_file".into(), true, "COMPLETED-OUTPUT".into());
    store.push_chunk(&chunk(ChatStreamKind::Content, "NORMAL-ANSWER"));
    store.finalize_live_tail();
    let completed = text(&store.render_fullscreen(100, &options(), &HashSet::new()));
    assert!(completed.contains("CURRENT-PROGRESS"));
    assert!(completed.contains("COMPLETED-OUTPUT"));
    assert!(completed.contains("CURRENT-THOUGHT"));
    assert!(completed.contains("NORMAL-ANSWER"), "{completed}");
    assert!(!completed
        .lines()
        .find(|line| line.contains("NORMAL-ANSWER"))
        .unwrap()
        .contains('│'));
    tui_style::set_override(None);
}

/// 【终端】【执行链测试】隐藏思考和工具时不显示节点与计数，审批操作则不能整体遮挡。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_respects_visibility_and_permission() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("PRIVATE-THOUGHT".into()));
    store.push_tool_call("run_command".into(), r#"{"command":"echo private"}"#.into());
    let hidden = TranscriptRenderOptions {
        reasoning_mode: crate::render::ReasoningDisplayMode::Hidden,
        tool_call_mode: crate::render::ToolCallDisplayMode::Hidden,
    };
    let document = store.render_fullscreen(100, &hidden, &HashSet::new());
    assert!(document.lines.is_empty());
    let HistoryCell::Tool(super::super::tool_cell::ToolCell::Invocation(view)) =
        &mut store.cells[1]
    else {
        panic!("expected pending tool")
    };
    view.request_permission("permission-id".into());
    let document = store.render_fullscreen(
        100,
        &options(),
        &HashSet::from([ParagraphKey::activity(0, true)]),
    );
    assert!(document.lines.len() > 5);
    assert!(text(&document).contains("private"));
    tui_style::set_override(None);
}

/// 【终端】【执行链测试】实时工具耗时在完成后冻结，恢复历史时保持未知。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_timing_is_real_and_not_fabricated_for_history() {
    let mut store = TranscriptStore::new(200);
    store.push_tool_call("read_file".into(), "{}".into());
    store.push_tool_result("read_file".into(), true, "done".into());
    let HistoryCell::Tool(super::super::tool_cell::ToolCell::Invocation(view)) = &store.cells[0]
    else {
        panic!("expected tool")
    };
    let elapsed = view.elapsed().unwrap();
    assert_eq!(view.elapsed().unwrap(), elapsed);
    store.push_history_tool_call("read_file".into(), "{}".into());
    store.push_tool_result("read_file".into(), true, "restored".into());
    let HistoryCell::Tool(super::super::tool_cell::ToolCell::Invocation(view)) = &store.cells[1]
    else {
        panic!("expected history tool")
    };
    assert_eq!(view.elapsed(), None);
}

/// 【终端】【执行链测试】窄窗口中标题、连接线和当前详情均不超过可用宽度。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_fits_narrow_widths() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = TranscriptStore::new(200);
    store.push_chunk(&chunk(
        ChatStreamKind::Reasoning,
        "中文详细信息".repeat(100).as_str(),
    ));
    for width in [1, 2, 3, 12, 30, 80] {
        let document = store.render_fullscreen(width, &options(), &HashSet::new());
        assert!(
            text(&document)
                .lines()
                .all(|line| unicode_width::UnicodeWidthStr::width(line) <= width),
            "width={width}, text={}",
            text(&document)
        );
    }
    tui_style::set_override(None);
}

/// 【终端】【执行链测试】颜色随真实结果切换，准备参数与等待审批的英文提示准确描述阶段。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_timeline_colors_and_labels_match_state() {
    use crate::render::status_style::ToolHealth;
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let mut store = TranscriptStore::new(200);
        store.push_tool_call("read_file".into(), r#"{"path":"fixture.rs"}"#.into());
        assert_eq!(appearance::health(&store.cells[0]), ToolHealth::Pending);
        let running = node::render_cell(&store.cells[0], 80, true, false);
        assert!(running.lines[0].as_str().contains("\x1b[33m•"));
        assert!(running.lines[0].as_str().contains("Reading file"));
        store.push_tool_result("read_file".into(), true, "done".into());
        let done = node::render_cell(&store.cells[0], 80, false, false);
        assert!(done.lines[0].as_str().contains("\x1b[32m•"));
        store.push_tool_call("run_command".into(), "{}".into());
        store.push_tool_result("run_command".into(), false, "error".into());
        let failed = node::render_cell(&store.cells[1], 80, false, false);
        assert!(failed.lines[0].as_str().contains("\x1b[31m"));
        assert!(failed.lines[0].as_str().contains("Failed"));
        store.push_tool_call("run_command".into(), "{}".into());
        let HistoryCell::Tool(super::super::tool_cell::ToolCell::Invocation(view)) =
            &mut store.cells[2]
        else {
            panic!("expected tool")
        };
        view.request_permission("pending".into());
        let pending = node::render_cell(&store.cells[2], 80, true, false);
        assert!(pending.lines[0].as_str().contains("Awaiting approval"));
        store.live_tool_call = Some(super::super::store::LiveToolCall {
            name: "read_file".into(),
            arguments_preview: "{}".into(),
            edit_diff_counts: None,
        });
        let preparing = node::render_live(&store, node::LiveNode::Tool, 80, false);
        assert!(preparing.lines[0].as_str().contains("Preparing tool call"));
        assert!(!preparing.lines[0].as_str().contains("Reading file"));
        let thought = appearance::title("◇ Thinking · 12 tokens", ToolHealth::Pending, true, 80);
        assert!(thought.as_str().contains("\x1b[36m◇"));
        assert!(thought.as_str().contains("\x1b[2m · 12 tokens\x1b[0m"));
    });
}

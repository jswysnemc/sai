use super::super::test_support::options;
use super::super::tool_cell::ToolCell;
use super::*;
use crate::config::TuiStyle;
use crate::render::{activity_animation::strip_ansi_for_test, tui_style};

/// 【终端】【旧步骤测试】构造连续的已完成工具，每个路径可独立识别。
/// 参数: `count` 为工具数量；返回: 保留所有源步骤的记录。
fn completed_tools(count: usize) -> TranscriptStore {
    let mut store = TranscriptStore::new(200);
    for index in 0..count {
        store.push_tool_call(
            "read_file".into(),
            serde_json::json!({"path": format!("step-{index:02}.txt")}).to_string(),
        );
        store.push_tool_result("read_file".into(), true, format!("1\tCONTENT-{index:02}"));
    }
    store
}

/// 【终端】【旧步骤测试】读取文档纯文本，排除颜色序列对断言的影响。
/// 参数: `document` 为渲染文档；返回: 按行拼接的可见内容。
fn text(document: &FullscreenDocument) -> String {
    strip_ansi_for_test(
        &document
            .lines
            .iter()
            .map(AnsiLine::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// 【终端】【旧步骤测试】长工具组保留末尾十项，窗口内的近期详情保持展开。
/// 参数: 无；返回: 无。
#[test]
fn completed_group_shows_latest_ten_and_folds_older_steps() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = completed_tools(12);
    let document = store.render_fullscreen(100, &options(), &HashSet::new());
    let plain = text(&document);
    assert!(plain.contains("12 reads"), "{plain}");
    assert!(plain.contains("Show 2 earlier steps"), "{plain}");
    for index in 0..2 {
        assert!(!plain.contains(&format!("step-{index:02}.txt")), "{plain}");
    }
    for index in 2..12 {
        assert!(plain.contains(&format!("step-{index:02}.txt")), "{plain}");
    }
    assert!(document.activities.iter().all(|region| region.scrollable));
    assert!(plain.contains("CONTENT-11"));
    assert!(!plain.contains("CONTENT-02"));
    assert_eq!(store.cells.len(), 12);
    tui_style::set_override(None);
}

/// 【终端】【旧步骤测试】不足十项的手动折叠组完整展示摘要，不生成多余入口或独立滚动。
/// 参数: 无；返回: 无。
#[test]
fn short_collapsed_group_has_no_older_steps_control_or_nested_scroll() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = completed_tools(3);
    let document = store.render_fullscreen(
        100,
        &options(),
        &HashSet::from([
            ParagraphKey::activity(0, false),
            ParagraphKey::activity(1, false),
            ParagraphKey::activity(2, false),
        ]),
    );
    assert!(!text(&document).contains("earlier steps"));
    assert!(document.activities.iter().all(|region| !region.scrollable));
    tui_style::set_override(None);
}

/// 【终端】【层级对齐测试】旧步骤入口无论展开或收起，都与分组标题及工具摘要使用相同起始列。
/// 参数: 无；返回: 无。
#[test]
fn older_steps_control_keeps_the_same_column_in_both_states() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = completed_tools(12);
    let key = ParagraphKey::older_activity(0);
    for expanded in [HashSet::new(), HashSet::from([key])] {
        let document = store.render_fullscreen(100, &options(), &expanded);
        let plain = text(&document);
        let group = plain
            .lines()
            .find(|line| line.contains("12 reads"))
            .unwrap();
        let control = plain
            .lines()
            .find(|line| line.contains("earlier steps"))
            .unwrap();
        let step = plain
            .lines()
            .find(|line| line.contains("step-11.txt"))
            .unwrap();
        let column = |line: &str| {
            line.chars()
                .take_while(|character| *character == ' ')
                .count()
        };
        assert_eq!(column(control), column(group) + 2, "{plain}");
        assert_eq!(column(control), column(step), "{plain}");
    }
    tui_style::set_override(None);
}

/// 【终端】【旧步骤测试】入口恢复旧步骤，收起和整组折叠都保留子步骤的展开选择。
/// 参数: 无；返回: 无。
#[test]
fn older_steps_toggle_preserves_child_expansion_and_group_count() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = completed_tools(12);
    let initial = store.render_fullscreen(100, &options(), &HashSet::new());
    let row = initial
        .lines
        .iter()
        .position(|line| line.as_str().contains("Show 2 earlier steps"))
        .unwrap();
    let control = initial
        .paragraphs
        .iter()
        .find(|span| span.start == row)
        .unwrap()
        .key;
    let child = ParagraphKey::whole(0);
    let expanded = HashSet::from([control, child]);
    let revealed = store.render_fullscreen(100, &options(), &expanded);
    assert!(text(&revealed).contains("Hide 2 earlier steps"));
    assert!(text(&revealed).contains("CONTENT-00"));
    let hidden = store.render_fullscreen(100, &options(), &HashSet::from([child]));
    assert!(!text(&hidden).contains("CONTENT-00"));
    assert!(hidden.retained_keys.contains(&child));
    assert!(hidden.retained_keys.contains(&control));
    let folded = store.render_fullscreen(
        100,
        &options(),
        &HashSet::from([control, child, ParagraphKey::activity(0, true)]),
    );
    assert_eq!(folded.lines.len(), 1);
    assert!(folded.retained_keys.contains(&control));
    assert!(folded.retained_keys.contains(&child));
    let restored = store.render_fullscreen(100, &options(), &expanded);
    assert!(text(&restored).contains("CONTENT-00"));
    assert!(text(&restored).contains("12 reads"));
    tui_style::set_override(None);
}

/// 【终端】【旧步骤测试】旧步骤的审批强制显示全部步骤，不允许历史入口或视口隐藏控件。
/// 参数: 无；返回: 无。
#[test]
fn approval_in_older_step_bypasses_folding_and_nested_scroll() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = completed_tools(12);
    let HistoryCell::Tool(ToolCell::Invocation(view)) = &mut store.cells[0] else {
        panic!("expected tool")
    };
    view.request_permission("pending".into());
    let document = store.render_fullscreen(
        100,
        &options(),
        &HashSet::from([ParagraphKey::activity(0, true)]),
    );
    assert!(text(&document).contains("Awaiting approval"));
    for index in 0..12 {
        assert!(text(&document).contains(&format!("step-{index:02}.txt")));
    }
    assert!(!text(&document).contains("earlier steps"));
    assert!(document.activities.iter().all(|region| !region.scrollable));
    tui_style::set_override(None);
}

/// 【终端】【旧步骤测试】被偏好隐藏的思考不计入末尾十项，也不出现在较早步骤数量中。
/// 参数: 无；返回: 无。
#[test]
fn recent_steps_count_only_visible_nodes() {
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let mut store = completed_tools(11);
    for index in 0..8 {
        store.push_cell(HistoryCell::reasoning(format!("HIDDEN-THOUGHT-{index}")));
    }
    let mut preferences = options();
    preferences.reasoning_mode = crate::render::ReasoningDisplayMode::Hidden;
    let document = store.render_fullscreen(100, &preferences, &HashSet::new());
    let plain = text(&document);
    assert!(plain.contains("Show 1 earlier step"));
    assert!(!plain.contains("step-00.txt"));
    for index in 1..11 {
        assert!(plain.contains(&format!("step-{index:02}.txt")));
    }
    assert!(!plain.contains("thought"));
    tui_style::set_override(None);
}

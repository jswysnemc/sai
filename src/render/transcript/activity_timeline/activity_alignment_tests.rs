use super::super::{cell::HistoryCell, fullscreen_parts::ParagraphPart, test_support::options};
use super::*;
use crate::config::TuiMode;
use crate::render::{activity_animation::strip_ansi_for_test, tui_mode};

/// 【终端】【执行链对齐测试】构造包含思考、命令和文件工具的混合分组。
/// 参数: 无；返回: 已完成步骤的记录。
fn mixed_steps() -> TranscriptStore {
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("Check the result".into()));
    store.push_tool_call("run_command".into(), r#"{"command":"echo result"}"#.into());
    store.push_tool_result(
        "run_command".into(),
        true,
        r#"{"success":true,"exit_code":0,"stdout":"result","stderr":""}"#.into(),
    );
    store.push_tool_call("read_file".into(), r#"{"path":"result.txt"}"#.into());
    store.push_tool_result("read_file".into(), true, "1\tresult".into());
    store
}

/// 【终端】【执行链对齐测试】不同工具符号不改变同层标题的起始列，窄窗口仍使用同一缩进。
/// 参数: 无；返回: 无。
#[test]
fn activity_headers_share_a_column_for_mixed_tools() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = mixed_steps();
    for width in [2, 20, 80] {
        let document = store.render_fullscreen(width, &options(), &HashSet::new());
        for span in &document.paragraphs {
            let plain = strip_ansi_for_test(document.lines[span.start].as_str());
            let content_width = width.saturating_sub(width.saturating_sub(1).min(2)).max(1);
            let inset = if span.key.part == ParagraphPart::ActivityGroup {
                0
            } else {
                content_width.saturating_sub(1).min(2)
            };
            assert_eq!(
                plain
                    .chars()
                    .take_while(|character| *character == ' ')
                    .count(),
                inset,
                "width={width}, part={:?}, line={plain:?}",
                span.key.part
            );
        }
    }
    tui_mode::set_override(None);
}

/// 【终端】【执行链对齐测试】整个分组收起后不因折叠箭头改变左侧层级。
/// 参数: 无；返回: 无。
#[test]
fn folded_activity_group_keeps_its_column() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = mixed_steps();
    for width in [2, 20, 80] {
        let document = store.render_fullscreen(
            width,
            &options(),
            &HashSet::from([ParagraphKey::activity(0, true)]),
        );
        let span = document
            .paragraphs
            .iter()
            .find(|span| span.key.part == ParagraphPart::ActivityGroup)
            .unwrap();
        let plain = strip_ansi_for_test(document.lines[span.start].as_str());
        assert_eq!(
            plain
                .chars()
                .take_while(|character| *character == ' ')
                .count(),
            0,
            "{plain:?}"
        );
    }
    tui_mode::set_override(None);
}

/// 【终端】【单步展示测试】只有一个步骤时直接展示全文，不生成分组标题，首行仍可折叠。
/// 参数: 无；返回: 无。
#[test]
fn single_step_is_expanded_without_a_redundant_group() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_cell(HistoryCell::reasoning("Single thought detail".into()));
    let document = store.render_fullscreen(
        80,
        &options(),
        &HashSet::from([ParagraphKey::activity(0, true)]),
    );
    let plain = document
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(plain.contains("Single thought detail"), "{plain}");
    assert!(!plain.contains("1 thought"), "{plain}");
    assert_eq!(document.paragraphs.len(), 1);
    assert_eq!(document.paragraphs[0].control, document.paragraphs[0].start);
    tui_mode::set_override(None);
}

/// 【终端】【执行链左移测试】用户、分组及单步标题使用同一引导列，子步骤仍保持两列层级。
/// 参数: 无；返回: 无。
#[test]
fn activity_group_and_single_step_align_with_user_marker() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_user_echo(super::super::TranscriptMode::Yolo, "USER-MARKER".into());
    store.push_cell(HistoryCell::reasoning("Inspect layout".into()));
    let single = store.render_fullscreen(80, &options(), &HashSet::new());
    let user = single
        .lines
        .iter()
        .find(|line| line.as_str().contains("USER-MARKER"))
        .unwrap();
    let step = single
        .lines
        .iter()
        .find(|line| strip_ansi_for_test(line.as_str()).contains("Thought"))
        .unwrap();
    assert_eq!(strip_ansi_for_test(user.as_str()).find('●'), Some(0));
    assert_eq!(
        strip_ansi_for_test(step.as_str())
            .chars()
            .take_while(|character| *character == ' ')
            .count(),
        0
    );
    store.push_tool_call("read_file".into(), r#"{"path":"result.txt"}"#.into());
    store.push_tool_result("read_file".into(), true, "result".into());
    let grouped = store.render_fullscreen(80, &options(), &HashSet::new());
    let group = grouped
        .paragraphs
        .iter()
        .find(|span| span.key.part == ParagraphPart::ActivityGroup)
        .unwrap();
    assert_eq!(
        strip_ansi_for_test(grouped.lines[group.start].as_str())
            .find(crate::render::guide_symbols::READ),
        Some(0)
    );
    tui_mode::set_override(None);
}

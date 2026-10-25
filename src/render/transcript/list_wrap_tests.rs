use super::{assistant_body, markdown_cell};
use crate::config::TuiStyle;
use crate::render::{activity_animation::strip_ansi_for_test, tui_style};

/// 【终端】【列表换行测试】在两种样式中，嵌套列表的自动续行与正文起始列对齐。
/// 参数: 无；返回: 无。
#[test]
fn nested_unordered_list_continuations_align_with_text() {
    for style in [TuiStyle::Classic, TuiStyle::Deepseek] {
        tui_style::set_override(Some(style));
        let source = format!(
            "Introduction\n  - Codex 实现：Linux {}\n  - next item",
            "跨平台沙箱能力与受限令牌隔离机制".repeat(4)
        );
        let rendered = markdown_cell::render_completed(&source);
        let lines: Vec<_> = assistant_body::display_lines(&rendered, 40)
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect();
        let first = lines
            .iter()
            .position(|line| line.contains("Codex"))
            .unwrap();
        let last = lines
            .iter()
            .position(|line| line.contains("next item"))
            .unwrap();
        assert!(last > first + 1);
        for line in &lines[first + 1..last] {
            assert_eq!(
                line.chars()
                    .take_while(|character| *character == ' ')
                    .count(),
                6,
                "{style:?}: {line:?}"
            );
        }
    }
    tui_style::set_override(None);
}

/// 【终端】【列表换行测试】两位数有序列表按实际标记宽度缩进，不固定为两列。
/// 参数: 无；返回: 无。
#[test]
fn numbered_list_continuations_account_for_marker_width() {
    tui_style::set_override(Some(TuiStyle::Classic));
    let rendered =
        markdown_cell::render_completed(&format!("10. {}", "ordered-content-".repeat(6)));
    let lines = assistant_body::display_lines(&rendered, 35);
    assert!(lines.len() > 1);
    for line in &lines[1..] {
        let plain = strip_ansi_for_test(line.as_str());
        assert_eq!(
            plain
                .chars()
                .take_while(|character| *character == ' ')
                .count(),
            6,
            "{plain:?}"
        );
    }
    tui_style::set_override(None);
}

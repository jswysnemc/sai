use super::{assistant_body, markdown_cell};
use crate::config::TuiMode;
use crate::render::{activity_animation::strip_ansi_for_test, tui_mode};

/// 【终端】【列表排版回归】真实正文路径用缩进区分层级，显式续行与项目正文对齐；无参数和返回值。
#[test]
fn lists_use_uniform_bullets_and_indent_source_continuations() {
    let source =
        "- **`check_os_info`**\n  - 测试方向一\n    确认全流程状态流转\n正确。\n- Next group";
    for style in [TuiMode::Inline, TuiMode::Fullscreen] {
        tui_mode::set_override(Some(style));
        let rendered = markdown_cell::render_completed(source);
        let lines = assistant_body::display_lines(&rendered, 40)
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>();
        assert!(rendered.contains('•'), "{lines:?}");
        assert!(!rendered.contains("38;5;183m"), "{rendered:?}");
        let child = lines
            .iter()
            .find(|line| line.contains("测试方向一"))
            .unwrap();
        let column =
            unicode_width::UnicodeWidthStr::width(&child[..child.find("测试方向一").unwrap()]);
        for text in ["确认全流程状态流转", "正确。"] {
            let line = lines.iter().find(|line| line.contains(text)).unwrap();
            assert_eq!(line.find(text), Some(column), "{lines:?}");
        }
        assert!(
            lines.iter().any(|line| line.contains("• check_os_info")),
            "{lines:?}"
        );
        assert!(child.starts_with("      • "), "{lines:?}");
        let next = lines
            .iter()
            .position(|line| line.contains("Next group"))
            .unwrap();
        assert!(lines[next - 1].trim().is_empty(), "{lines:?}");
    }
    tui_mode::set_override(None);
}

/// 【终端】【列表换行测试】在两种样式中，嵌套列表的自动续行与正文起始列对齐。
/// 参数: 无；返回: 无。
#[test]
fn nested_unordered_list_continuations_align_with_text() {
    for style in [TuiMode::Inline, TuiMode::Fullscreen] {
        tui_mode::set_override(Some(style));
        let source = format!(
            "Introduction\n- Parent\n  - Codex 实现：Linux {}\n  - next item",
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
                8,
                "{style:?}: {line:?}"
            );
        }
    }
    tui_mode::set_override(None);
}

/// 【终端】【列表换行测试】两位数有序列表按实际标记宽度缩进，不固定为两列。
/// 参数: 无；返回: 无。
#[test]
fn numbered_list_continuations_account_for_marker_width() {
    tui_mode::set_override(Some(TuiMode::Inline));
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
    tui_mode::set_override(None);
}

/// 【终端】【列表换行测试】断点落在词间空格时续行对齐正文列，多空格内容保持原样。
/// 参数: 无；返回: 无。
#[test]
fn list_wrap_drops_single_break_space() {
    use crate::render::style::{MD_LIST_MARKER_STYLE, RESET};
    let wrapped = |text: &str| {
        super::list_wrap::wrap(&format!("{MD_LIST_MARKER_STYLE}•{RESET} {text}"), 6, 0)
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>()
    };
    assert_eq!(wrapped("abcd efgh"), ["• abcd", "  efgh"]);
    assert_eq!(wrapped("abcd  ef"), ["• abcd", "    ef"]);
}

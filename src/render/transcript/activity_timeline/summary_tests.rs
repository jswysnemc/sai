use super::super::{cell::HistoryCell, shell_cell::ShellCell, tool_cell::ToolCell};
use super::node;
use crate::render::tool_view::ToolView;
use unicode_width::UnicodeWidthStr;

/// 【终端】【分组图标测试】分组标题使用汇总图标，子项和后续分类图标保持原样。
/// 参数：无；返回：无。
#[test]
fn group_guide_is_distinct_from_individual_tool_icons() {
    use crate::render::guide_symbols as icons;
    for first in [icons::READ, icons::TOOL, icons::THOUGHT] {
        let suffix = format!(" 16 steps · {} 2 thoughts", icons::THOUGHT);
        let title = format!("{first}{suffix}");
        let rendered = super::appearance::group_title(&title, 80);
        let plain = crate::render::ansi_clip::strip_ansi(rendered.as_str());
        assert_eq!(plain, format!("\u{f03a}{suffix}"));
        for width in [1, 8, 24] {
            let rendered = super::appearance::group_title(&title, width);
            let plain = crate::render::ansi_clip::strip_ansi(rendered.as_str());
            assert!(plain.starts_with('\u{f03a}'));
            assert!(UnicodeWidthStr::width(plain.as_str()) <= width);
        }
    }
    assert!(title(&read_cell("fixture.rs", "1\tcontent"), 80).starts_with(icons::READ));
}

/// 【终端】【工具图标测试】读取、执行、搜索与变更使用各自语义图标且均占一列。
/// 参数: 无；返回: 无。
#[test]
fn tools_use_distinct_semantic_guides() {
    use crate::render::guide_symbols as icons;
    for (name, expected) in [
        ("read_file", icons::READ),
        ("mcp__read_file", icons::READ),
        ("run_command", icons::COMMAND),
        ("background_command", icons::COMMAND),
        ("grep", icons::SEARCH),
        ("glob", icons::SEARCH),
        ("write_file", icons::EDIT),
        ("custom_tool", icons::TOOL),
        ("ask_question", icons::ASK),
        ("trash_path", icons::TRASH),
    ] {
        assert_eq!(super::summary::tool_icon(name), expected, "{name}");
        assert_eq!(UnicodeWidthStr::width(expected), 1);
    }
}

/// 【终端】【摘要测试】读取折叠节点标题，排除颜色序列对断言的影响。
/// 参数: `cell` 为步骤，`width` 为显示列数；返回: 单行纯文本标题。
fn title(cell: &HistoryCell, width: usize) -> String {
    let node = node::render_cell(cell, width, false, false);
    crate::render::activity_animation::strip_ansi_for_test(node.lines[0].as_str())
}

/// 【终端】【摘要测试】构造文件读取结果，保留请求范围与返回范围之间的差异。
/// 参数: `path` 为请求路径，`output` 为返回文本；返回: 已完成工具步骤。
fn read_cell(path: &str, output: &str) -> HistoryCell {
    let mut view = ToolView::running(
        "read_file".into(),
        serde_json::json!({"path": path, "offset": 1, "limit": 20}).to_string(),
    );
    view.start_timer();
    view.finish(true, output.into());
    HistoryCell::Tool(ToolCell::Invocation(Box::new(view)))
}

/// 【终端】【摘要测试】长目录不能遮挡文件名，完成摘要必须使用实际返回范围。
/// 参数: 无；返回: 无。
#[test]
fn narrow_read_summary_keeps_filename_and_actual_range() {
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let cell = read_cell(
            "a-long-project-directory/another-long-directory/source-file.rs",
            "1\tfirst\n2\tsecond\n3\tthird",
        );
        let summary = title(&cell, 48);
        assert!(summary.contains("source-file.rs (L1–3)"), "{summary}");
        assert!(!summary.contains("L1–20"), "{summary}");
        assert!(UnicodeWidthStr::width(summary.as_str()) <= 48);
        let compact = title(&cell, 38);
        assert!(compact.contains("source-file.rs (L1–3)"), "{compact}");
        assert!(!compact.contains("ms"), "{compact}");
    });
}

/// 【终端】【摘要测试】宽终端保留完整路径，历史分页采用返回路径和实际起始行。
/// 参数: 无；返回: 无。
#[test]
fn wide_read_summary_preserves_path_and_legacy_range() {
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let output = serde_json::json!({"type": "text-page", "path": "actual/source.rs",
            "offset": 12, "content": "first\nsecond", "truncated": false})
        .to_string();
        let cell = read_cell("requested/source.rs", &output);
        let summary = title(&cell, 100);
        assert!(summary.contains("actual/source.rs (L12–13)"), "{summary}");
        assert!(!summary.contains("requested/source.rs"), "{summary}");
    });
}

/// 【终端】【摘要测试】空结果和非分页结果不冒充已经返回请求范围。
/// 参数: 无；返回: 无。
#[test]
fn completed_read_without_page_omits_requested_range() {
    for output in ["", "Directory listing", "No lines returned"] {
        assert!(!title(&read_cell("source.rs", output), 100).contains("L1–20"));
    }
}

/// 【终端】【摘要测试】多字节文件名裁剪不超出列宽，并保留后缀与行范围。
/// 参数: 无；返回: 无。
#[test]
fn narrow_read_summary_preserves_unicode_suffix() {
    let cell = read_cell("很长的目录/非常长的文件名称用于展示.rs", "1\tfirst");
    for width in [28, 32, 40] {
        let summary = title(&cell, width);
        assert!(summary.ends_with(".rs (L1)"), "{summary}");
        assert!(UnicodeWidthStr::width(summary.as_str()) <= width);
    }
    for width in 1..60 {
        assert!(UnicodeWidthStr::width(title(&cell, width).as_str()) <= width);
    }
}

/// 【终端】【摘要测试】失败与审批状态优先于长参数，不依赖颜色辨认结果。
/// 参数: 无；返回: 无。
#[test]
fn narrow_tool_summary_keeps_failure_and_approval_visible() {
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let arguments = serde_json::json!({"command": "printf sample ".repeat(30)}).to_string();
        let mut failed = ToolView::running("run_command".into(), arguments.clone());
        failed.finish(false, "exit status: 7".into());
        let summary = title(
            &HistoryCell::Tool(ToolCell::Invocation(Box::new(failed))),
            48,
        );
        assert!(summary.contains("Failed"), "{summary}");
        let mut pending = ToolView::running("run_command".into(), arguments);
        pending.request_permission("request".into());
        let summary = title(
            &HistoryCell::Tool(ToolCell::Invocation(Box::new(pending))),
            48,
        );
        assert!(summary.contains("Awaiting approval"), "{summary}");
        let shell = HistoryCell::Shell(ShellCell {
            command: "printf sample ".repeat(30),
            output: String::new(),
            exit_code: Some(7),
        });
        let summary = title(&shell, 48);
        assert!(summary.contains("Failed (exit 7)"), "{summary}");
    });
}

/// 【终端】【摘要测试】命令采用结果中的成功标志，不把工具传输成功误认成命令执行成功。
/// 参数: 无；返回: 无。
#[test]
fn command_summary_uses_command_success_instead_of_transport_success() {
    use crate::render::status_style::ToolHealth;
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        for success in [false, true] {
            let mut view = ToolView::running(
                "run_command".into(),
                serde_json::json!({"command": "printf sample ".repeat(30)}).to_string(),
            );
            view.finish(
                true,
                serde_json::json!({"success": success,
                "exit_code": if success { 0 } else { 7 },
                "stdout": "sample", "stderr": "diagnostic"})
                .to_string(),
            );
            let cell = HistoryCell::Tool(ToolCell::Invocation(Box::new(view)));
            assert_eq!(
                super::appearance::health(&cell),
                if success {
                    ToolHealth::Ok
                } else {
                    ToolHealth::Err
                }
            );
            let summary = title(&cell, 48);
            assert_eq!(summary.contains("Failed"), !success, "{summary}");
            let node = node::render_cell(&cell, 48, false, false);
            let health = if success {
                ToolHealth::Ok
            } else {
                ToolHealth::Err
            };
            assert!(node.lines[0].as_str().contains(
                &crate::render::status_style::tool_bullet(health)
                    .replace("•", crate::render::guide_symbols::COMMAND)
            ));
        }
    });
}

/// 【终端】【摘要测试】失败编辑与待审批编辑使用明确文字和可读动作。
/// 参数: 无；返回: 无。
#[test]
fn diff_summary_uses_action_and_explicit_state() {
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let mut cell = super::super::diff_cell::DiffCell::from_call(
            "write_file".into(),
            serde_json::json!({"path": "source.rs", "content": "first"}).to_string(),
        );
        let mut approval = cell.clone();
        approval.request_permission("request".into());
        let pending = title(&HistoryCell::Diff(approval), 48);
        assert!(pending.contains("Awaiting approval"), "{pending}");
        cell.finish(false);
        let failed = title(&HistoryCell::Diff(cell), 48);
        assert!(failed.contains("Failed"), "{failed}");
        assert!(!failed.contains("write_file"), "{failed}");
    });
}

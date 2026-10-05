use super::*;
use crate::render::ToolCallDisplayMode;

#[test]
fn lifecycle_view_replaces_call_with_result() {
    let mut view = ToolView::running(
        "read_file".to_string(),
        r#"{"path":"README.md"}"#.to_string(),
    );
    view.set_progress("reading file".to_string());
    view.finish(true, "contents".to_string());

    let output = render(&view, ToolCallDisplayMode::Full);

    assert!(output.contains("README.md"));
    assert!(output.contains("reading file"));
    assert!(output.contains("contents"));
    // 统一 gutter：`  └ ` 首行 + 四空格续行，不再使用 `└─`/`├─`
    assert!(output.contains("  └ "));
    assert!(!output.contains("└─") && !output.contains("├─"));
}

#[test]
fn summary_view_keeps_failure_visible() {
    let output = render_result(
        "read_file",
        false,
        "permission denied",
        ToolCallDisplayMode::Summary,
    );

    assert!(!output.is_empty());
    assert!(output.contains("failed"));
}

/// 清单标题只显示一次计划进度，正文保留条目内容和状态。
#[test]
fn todo_result_renders_items_instead_of_raw_json() {
    let output = render_result(
        "todo",
        true,
        r#"{"ok":true,"items":[{"id":"1","text":"检查测试","status":"completed"},{"id":"2","text":"构建项目","status":"in_progress"}]}"#,
        ToolCallDisplayMode::Full,
    );

    assert!(output.contains("检查测试"));
    assert!(output.contains("构建项目"));
    assert!(output.lines().next().unwrap_or_default().contains("1/2"));
    assert_eq!(output.matches("Todo").count(), 1);
    assert!(output.contains('✓') && output.contains('◐'));
    // 1. 进度合并到标题，条目自带状态符，不再重复统计行
    assert!(!output.contains('├') && !output.contains("└─"));
    assert!(!output.contains("\"items\""));
}

/// 验证命令审计选择附着在既有命令块下方。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn command_permission_uses_existing_command_view() {
    let mut view = ToolView::running(
        "run_command".to_string(),
        r#"{"command":"cargo test"}"#.to_string(),
    );
    view.request_permission("permission".to_string());

    let output = render(&view, ToolCallDisplayMode::Full);

    assert!(output.contains("cargo"));
    assert!(output.contains("test"));
    assert!(output.contains("Allow once"));
    assert!(!output.contains("Permission required"));
}

/// 后台命令结果使用自然状态摘要，不展示内部 JSON。
#[test]
fn background_command_result_uses_command_view() {
    let output = render_result(
        "background_command",
        true,
        r#"{"ok":true,"task":{"id":"task-1","status":"running"}}"#,
        ToolCallDisplayMode::Full,
    );

    assert!(output.contains("task-1"));
    assert!(!output.contains("── • Run command"));
    assert!(!output.contains("\"task\""));
    assert!(output.contains("Running"));
    assert!(output.contains("in background"));
}

/// Full 模式在参数 JSON 尚未闭合时不倾倒 `{...` 碎片。
#[test]
fn full_view_hides_incomplete_json_arguments() {
    let view = ToolView::running("web_search".to_string(), r#"{"query":"partial"#.to_string());
    let output = render(&view, ToolCallDisplayMode::Full);
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);
    assert!(!plain.contains("└ args:"));
    assert!(!plain.contains("{\"query\""));
}

/// 编辑类工具在出结果前用实时增删统计顶替 run 动效，数字随参数流跳动。
#[test]
fn edit_tool_status_line_shows_streamed_diff_stats() {
    let view = ToolView::running(
        "write_file".to_string(),
        r#"{"path":"a.rs","content":"l1\nl2\nl3\nl4"#.to_string(),
    );

    let output = super::render(&view, ToolCallDisplayMode::Summary);

    assert!(output.contains("\x1b[32m+4\x1b[0m"));
    assert!(output.contains("\x1b[31m-0\x1b[0m"));
    assert!(!output.contains("run"));
}

/// 工具成功定稿后仍保留 +N -M，与参数流阶段的跳动数字衔接。
#[test]
fn edit_tool_result_keeps_diff_stats_on_status_line() {
    let mut view = ToolView::running(
        "write_file".to_string(),
        r#"{"path":"a.rs","content":"l1\nl2"}"#.to_string(),
    );
    view.finish(
        true,
        r#"{"changed_files":[{"path":"a.rs","added":2,"removed":0}]}"#.to_string(),
    );

    let output = super::render(&view, ToolCallDisplayMode::Summary);

    assert!(output.contains("\x1b[32m+2\x1b[0m"));
    assert!(output.contains("\x1b[31m-0\x1b[0m"));
    assert!(!output.contains(" ok"));
    assert!(!output.contains("Added"));
}

/// 能力申请只列出暴露的名称，不倾倒工具 Schema。
#[test]
fn capability_result_lists_exposed_names() {
    let output = render_result(
        "request_capability",
        true,
        r#"{"ok":true,"router":"jev","tools":[{"name":"web_search","definition":{"type":"function","function":{"name":"web_search","description":"Search.","parameters":{"type":"object"}}}}],"skills":[{"name":"drawio","status":"loaded","content":"skill body marker"}],"instruction":"exposed"}"#,
        ToolCallDisplayMode::Full,
    );
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);

    assert!(plain.contains("Requested"), "{plain}");
    assert!(plain.contains("1 tool · 1 skill"), "{plain}");
    assert!(plain.contains("tool web_search"), "{plain}");
    assert!(plain.contains("skill drawio"), "{plain}");
    assert!(!plain.contains("parameters"), "{plain}");
    assert!(!plain.contains("skill body marker"), "{plain}");
}

/// 非编辑类工具的状态行不受实时统计影响。
#[test]
fn non_edit_tools_keep_a_plain_status_line() {
    let view = ToolView::running("grep".to_string(), r#"{"pattern":"a\nb"}"#.to_string());

    let output = super::render(&view, ToolCallDisplayMode::Summary);
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);

    assert!(plain.contains("Searching"), "{plain}");
    assert!(!plain.contains("preparing"), "{plain}");
    assert!(!output.contains("\x1b[32m+"));
}

/// 【终端提问】【状态回归】提问过程使用专用动词，完成后转为过去时；无参数和返回值。
#[test]
fn ask_question_uses_question_status_instead_of_running() {
    let mut view = ToolView::running("ask_question".into(), "{}".into());
    let waiting = super::formatter::render(&view, crate::render::ToolCallDisplayMode::Summary);
    let waiting = crate::render::activity_animation::strip_ansi_for_test(&waiting);
    assert!(waiting.contains("Asking"), "{waiting}");
    assert!(!waiting.contains("Running") && !waiting.contains("ask_question"));
    view.finish(true, "{}".into());
    let completed = super::formatter::render(&view, crate::render::ToolCallDisplayMode::Summary);
    let completed = crate::render::activity_animation::strip_ansi_for_test(&completed);
    assert!(completed.contains("Asked"));
}

/// 网页搜索定稿后挂结果条数与供应商徽标。
#[test]
fn web_search_summary_shows_result_count_and_provider() {
    let mut view = ToolView::running("web_search".into(), r#"{"query":"rust"}"#.into());
    view.finish(
        true,
        "## Search results for: rust\n**Provider**: Brave\n\n### 1. A\n**URL**: https://a\n\n### 2. B\n"
            .into(),
    );
    let plain = crate::render::activity_animation::strip_ansi_for_test(&super::render(
        &view,
        ToolCallDisplayMode::Summary,
    ));
    assert_eq!(
        plain.trim(),
        "• Searched the web for rust 2 results · Brave"
    );
}

/// 文件内容搜索定稿后挂命中行数与文件数，截断时标 `+`。
#[test]
fn file_search_summary_shows_match_and_file_counts() {
    let mut view = ToolView::running("grep".into(), r#"{"pattern":"main"}"#.into());
    view.finish(
        true,
        serde_json::json!({
            "success": true,
            "stdout": "src/a.rs:1:fn main\nsrc/a.rs:9:main()\nsrc/b.rs:3:main",
            "truncated": true
        })
        .to_string(),
    );
    let plain = crate::render::activity_animation::strip_ansi_for_test(&super::render(
        &view,
        ToolCallDisplayMode::Summary,
    ));
    assert_eq!(
        plain.trim(),
        "• Searched files for main 3+ matches in 2 files"
    );
    // 无命中时明确写 no matches
    let mut empty = ToolView::running("grep".into(), r#"{"pattern":"zzz"}"#.into());
    empty.finish(true, r#"{"stdout":"","truncated":false}"#.into());
    let plain = crate::render::activity_animation::strip_ansi_for_test(&super::render(
        &empty,
        ToolCallDisplayMode::Summary,
    ));
    assert!(plain.ends_with("no matches"), "{plain}");
}

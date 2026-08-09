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
    assert!(output.contains("└─"));
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
    assert!(output.contains("err"));
}

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
    assert!(output.contains("done") || output.contains("/"));
    assert!(output.contains("✓") || output.contains("›"));
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

/// 验证后台命令结果按普通工具载荷展示，不复用前台命令输出块。
#[test]
fn background_command_result_uses_tool_payload_view() {
    let output = render_result(
        "background_command",
        true,
        r#"{"ok":true,"task":{"id":"task-1","status":"running"}}"#,
        ToolCallDisplayMode::Full,
    );

    assert!(output.contains("task-1"));
    assert!(!output.contains("── • Run command"));
    assert!(!output.contains("Ctrl+O"));
}

/// 编辑类工具在参数流与执行阶段带写入行数后缀。
#[test]
fn edit_tool_status_line_shows_streamed_line_count() {
    let view = ToolView::running(
        "write_file".to_string(),
        r#"{"path":"a.rs","content":"l1\nl2\nl3\nl4"#.to_string(),
    );

    let output = super::render(&view, ToolCallDisplayMode::Summary);

    assert!(output.contains("写入 3 行") || output.contains("writing 3 lines"));
}

/// 工具出结果后不再展示写入行数，交给 diff 统计表达。
#[test]
fn edit_tool_result_drops_the_line_count_suffix() {
    let mut view = ToolView::running(
        "write_file".to_string(),
        r#"{"path":"a.rs","content":"l1\nl2"}"#.to_string(),
    );
    view.finish(true, r#"{"changed_files":[]}"#.to_string());

    let output = super::render(&view, ToolCallDisplayMode::Summary);

    assert!(!output.contains("写入") && !output.contains("writing"));
}

/// 非编辑类工具的状态行不受行数后缀影响。
#[test]
fn non_edit_tools_keep_a_plain_status_line() {
    let view = ToolView::running(
        "grep".to_string(),
        r#"{"pattern":"a\nb"}"#.to_string(),
    );

    let output = super::render(&view, ToolCallDisplayMode::Summary);

    assert!(!output.contains("写入") && !output.contains("writing"));
}

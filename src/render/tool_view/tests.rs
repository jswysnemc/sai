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
    assert!(output.contains(crate::i18n::text("Allow once", "允许一次")));
    assert!(!output.contains(crate::i18n::text("Permission required", "需要权限确认")));
}

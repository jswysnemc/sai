use super::*;
use crate::render::terminal_text as t;

/// 验证命令全文保留多行内容和较长参数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn command_full_text_keeps_multiline_and_long_command() {
    assert_eq!(
        tool_command_full_text(
            "run_command",
            Some(r#"{"command":"cargo test\ncargo build"}"#)
        )
        .as_deref(),
        Some("cargo test\ncargo build")
    );
    let long = "a".repeat(80);
    let args = format!(r#"{{"command":"{long}"}}"#);
    assert_eq!(
        tool_command_full_text("run_command", Some(&args)).as_deref(),
        Some(long.as_str())
    );
    let title = tool_command_title_colored("run_command", Some(r#"{"command":"echo hello"}"#));
    assert!(title.contains("Run"));
    assert!(title.contains("echo") || title.contains("\x1b["));
}

/// 验证前台与后台命令使用对应的运行标签。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn command_tools_use_run_label() {
    assert_eq!(
        tool_event_label("run_command", Some(r#"{"command":"date"}"#)),
        "Run date"
    );
    // 参数尚未闭合时宽松提取已收到的命令内容
    assert_eq!(
        tool_event_label("run_command", Some(r#"{"command":"cargo bui"#)),
        "Run cargo bui"
    );
    // 多行命令只展示首个非空行
    assert_eq!(
        tool_event_label(
            "run_command",
            Some(r#"{"command":"cargo test\ncargo build"}"#)
        ),
        "Run cargo test"
    );
    assert_eq!(
        tool_event_label(
            "background_command",
            Some(r#"{"action":"start","command":"sleep 1"}"#)
        ),
        format!("{} sleep 1", t("Background start", "启动后台命令"))
    );
    assert_eq!(
        tool_event_label("background_command", Some(r#"{"action":"list"}"#)),
        t("Background list", "后台命令列表")
    );
}

/// 验证文件工具标签包含目标文件名。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn file_tools_include_basename() {
    assert_eq!(
        tool_event_label(
            "edit_file",
            Some(
                r#"{"patch":"*** Begin Patch\n*** Update File: src/render/stream.rs\n@@\n-a\n+b\n*** End Patch"}"#
            )
        ),
        "Edit stream.rs"
    );
    assert_eq!(
        tool_event_label(
            "read_file",
            Some(r#"{"files":[{"path":"src/a.rs"},{"path":"src/b.rs"}]}"#)
        ),
        "Read a.rs b.rs"
    );
}

/// 验证未闭合参数仍可提取工具目标。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn partial_arguments_extract_target() {
    assert_eq!(
        tool_event_label(
            "edit_file",
            Some(r#"{"patch":"*** Begin Patch\n*** Update File: src/main.rs\n@@\n-unfinished"#)
        ),
        "Edit main.rs"
    );
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_search"#)),
        "Load tool web_search"
    );
    assert_eq!(
        tool_event_label("write_file", Some(r#"{"path":"src/main.rs","content":"fn "#)),
        "Write main.rs"
    );
}

/// 验证渐进加载工具使用资源类型和名称标签。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn load_uses_load_label() {
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_search"]}"#)),
        "Load tool web_search"
    );
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_fetch"]}"#)),
        "Load tool web_fetch"
    );
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"skill","keywords":["yce"]}"#)),
        "Load skill yce"
    );
}

/// 验证子智能体工具优先展示描述和操作目标。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn subagent_uses_description_label() {
    assert_eq!(
        tool_event_label("subagent", Some(r#"{"description":"scan code"}"#)),
        "Subagent scan code"
    );
    assert_eq!(
        tool_event_label(
            "subagent",
            Some(r#"{"action":"status","subagent_id":"subagent_1"}"#)
        ),
        "Subagent status subagent_1"
    );
}

/// 验证管理类工具标签包含操作和目标。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn management_tools_include_action_and_target() {
    assert_eq!(
        tool_event_label("todo", Some(r#"{"action":"add","text":"检查测试"}"#)),
        "Todo add 检查测试"
    );
    assert_eq!(
        tool_event_label("cron", Some(r#"{"action":"remove","id":"cron_1"}"#)),
        "Schedule remove cron_1"
    );
}

/// 验证事件文本不再附加冗余工具前缀。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn event_text_omits_tool_prefix() {
    let output = tool_event_text("Write main.rs", "ok");

    assert!(output.starts_with("• Write main.rs "));
    assert!(!output.contains("tool:"));
}

/// 验证未知工具使用通用工具标签。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn unknown_tools_use_tool_label() {
    let label = tool_event_label("custom_tool", Some(r#"{"value":1}"#));
    let output = tool_event_text(&label, "err");

    assert_eq!(label, "Tool custom_tool");
    assert!(output.contains("Tool custom_tool"));
    assert!(output.contains("\x1b[31merr\x1b[0m"));
}

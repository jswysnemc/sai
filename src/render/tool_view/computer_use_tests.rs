use super::{render, ToolView};
use crate::render::{activity_animation::strip_ansi_for_test, ToolCallDisplayMode};

/// 【终端】【桌面语义测试】观察结果展示控件和截图，不重复目标、暴露完整快照或打印 JSON。
/// 参数: 无；返回: 无。
#[test]
fn computer_use_observation_has_a_semantic_header_and_compact_controls() {
    let mut view = ToolView::running(
        "computer_screenshot".into(),
        r#"{"app":"42","window_id":101,"scale":0.5}"#.into(),
    );
    view.finish(true, "Computer Use state\napp: 42\nwindow_id: 101\nsnapshot_id: 12345678-private\n<app_state>\n{\"app_name\":\"Fixture\"}\n{\"element_index\":\"g1:0\",\"role\":\"button\",\"label\":\"Apply\",\"enabled\":true}\n</app_state>\nframe_id: 12345678-private\nScreenshot: 50x30; use pixels from this exact image.\nEach input consumes this snapshot.".into());
    let full = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(
        full.contains("Captured window · app 42 · window 101"),
        "{full}"
    );
    assert!(full.contains("• button Apply [g1:0]"), "{full}");
    assert!(full.contains("Screenshot: 50x30"));
    assert!(full.contains("Frame: 12345678"));
    assert!(!full.contains("{\""));
    assert!(!full.contains("<app_state>"));
    assert!(!full.contains("private"));
    assert!(!full.contains("Each input consumes"));
    let summary = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Summary));
    assert!(summary.contains("Captured window"));
    assert!(!summary.contains("Apply"));
    assert!(render(&view, ToolCallDisplayMode::Hidden).is_empty());
}

/// 【终端】【桌面失败测试】失败与审批状态置于标题开头，错误正文仍可展开阅读。
/// 参数: 无；返回: 无。
#[test]
fn computer_use_failure_and_approval_are_visible_in_summary() {
    let mut view = ToolView::running(
        "computer_use".into(),
        r#"{"action":"click","app":"42","window_id":101,"element_index":"g1:0"}"#.into(),
    );
    view.request_permission("desktop-permission".into());
    assert!(
        strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Summary))
            .contains("Awaiting approval")
    );
    view.finish(false, "stale_snapshot: observe again".into());
    let full = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(full.contains("Failed · Clicking control"));
    assert!(full.contains("stale_snapshot"));
    assert!(!full.contains("\"element_index\""));
}

/// 【终端】【结果标题测试】统一入口缺少原参数时使用中性标题，不能把输入误报为配置查询。
/// 参数: 无；返回: 无。
#[test]
fn computer_use_result_without_arguments_does_not_guess_the_action() {
    let result = strip_ansi_for_test(&super::render_result(
        "computer_use",
        true,
        "Input applied",
        ToolCallDisplayMode::Full,
    ));
    assert!(result.contains("Completed desktop action"), "{result}");
    assert!(result.contains("Input applied"));
    assert!(!result.contains("configuration"));
    let named = strip_ansi_for_test(&super::render_result(
        "computer_click",
        true,
        "Input applied",
        ToolCallDisplayMode::Summary,
    ));
    assert!(named.contains("Clicked control"), "{named}");
}

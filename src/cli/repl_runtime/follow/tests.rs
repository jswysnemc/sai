use super::*;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::TranscriptRenderOptions;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use serde_json::json;

/// 【终端】【远端工具回放】跟随端必须保留工具参数和结果，不得降级为只有名称的提示。
/// 参数: 无
/// 返回: 无，参数或结果缺失时断言失败
#[test]
fn regression_followed_tools_preserve_arguments_and_result() {
    let mut runtime = ReplRuntime::new(
        200,
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Hidden,
            tool_call_mode: ToolCallDisplayMode::Full,
        },
    );
    // 1. 测试只检查事件组装结果，冻结向真实终端输出
    runtime.reflow.schedule_immediate();
    for (kind, payload) in [
        (
            "tool.call.started",
            json!({
                "tool_id": "tool-1", "name": "write_memory",
                "arguments": "{\"content\":\"remember-original-content\"}",
            }),
        ),
        (
            "tool.result",
            json!({
                "tool_id": "tool-1", "name": "write_memory", "ok": true,
                "output": "memory-saved-result",
            }),
        ),
    ] {
        runtime
            .render_follow_event(WebEvent::new("run", "workspace", "session", kind, payload))
            .unwrap();
    }
    let lines = runtime.expanded_transcript_lines(120);
    let text = lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("remember-original-content"),
        "跟随工具丢失参数: {text}"
    );
    assert!(
        text.contains("memory-saved-result"),
        "跟随工具丢失结果: {text}"
    );
}

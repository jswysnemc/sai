use super::*;
use crate::render::{activity_animation::strip_ansi_for_test, ToolCallDisplayMode};

/// 【终端】【后台通知测试】使用内部通知工具生成纯文本，便于验证语义字段和标题。
/// 参数: `source` 为通知正文，`mode` 为展示方式；返回: 去除 ANSI 的工具文本。
fn notice(source: &str, mode: ToolCallDisplayMode) -> String {
    let mut view = ToolView::running("background_notice".into(), String::new());
    view.finish(true, source.into());
    strip_ansi_for_test(&render(&view, mode))
}

/// 【终端】【后台通知测试】中文内部命令通知转换为英文提示，摘要仍保留命令标识。
/// 参数: 无；返回: 无。
#[test]
fn command_notice_has_semantic_fields_and_identifiable_summary() {
    let source = "后台工作已完成，自动继续当前对话\n\n后台命令：echo done（task-1）\n状态：stopped\n说明：日志未附带，请使用 background_command action=output 读取";
    let full = notice(source, ToolCallDisplayMode::Full);
    assert!(full.contains("Background command stopped"), "{full}");
    assert!(full.contains("Command: echo done（task-1）"), "{full}");
    assert!(full.contains("Status: stopped"), "{full}");
    assert!(full.contains("Note: Logs are not attached."), "{full}");
    assert!(!full.contains("后台"), "{full}");
    assert_eq!(full.matches("echo done").count(), 1);
    let summary = notice(source, ToolCallDisplayMode::Summary);
    assert!(
        summary.contains("Background command stopped · echo done"),
        "{summary}"
    );
    assert!(!summary.contains("\n"));
    assert!(notice(source, ToolCallDisplayMode::Hidden).is_empty());
}

/// 【终端】【后台通知测试】子代理回执使用任务语义，不泄漏内部模型指令。
/// 参数: 无；返回: 无。
#[test]
fn persisted_subagent_notice_hides_control_instructions() {
    let source = "<external-completion-events>\n以下后台工作已经结束。请消费这些状态回执：\n\n子 Agent：Check fixtures（agent-1）\n状态：idle\n说明：持久子代理已完成当前任务段并进入待命\n</external-completion-events>";
    let full = notice(source, ToolCallDisplayMode::Full);
    assert!(full.contains("Subagent idle"), "{full}");
    assert!(
        full.contains("Subagent: Check fixtures（agent-1）"),
        "{full}"
    );
    assert!(full.contains("action=result"), "{full}");
    assert!(!full.contains("以下后台"));
    assert!(!full.contains("<external-completion-events>"));
}

/// 【终端】【后台通知测试】网格通知作为工具显示，保留正文且使用英文静态标签。
/// 参数: 无；返回: 无。
#[test]
fn mesh_notice_keeps_message_without_a_user_echo() {
    let source = "<mesh-message>\n以下网格通信已经送达。请消费这条消息：\n\n来自：agent-1\n目标：main\ncorrelation_id：message-1\n正文：\n原始消息内容\n</mesh-message>";
    let full = notice(source, ToolCallDisplayMode::Full);
    assert!(full.contains("Mesh message received"), "{full}");
    assert!(full.contains("From: agent-1"), "{full}");
    assert!(full.contains("Correlation ID: message-1"), "{full}");
    assert!(full.contains("原始消息内容"), "{full}");
    assert!(!full.contains("以下网格"));
}

/// 【终端】【后台通知测试】包装检测需要完整起止标记，避免把截断内容误判为通知。
/// 参数: 无；返回: 无。
#[test]
fn external_notice_requires_a_complete_wrapper() {
    assert!(is_external_notice(
        " <external-completion-events>done</external-completion-events>\n"
    ));
    assert!(is_external_notice("<mesh-message>message</mesh-message>"));
    assert!(!is_external_notice(
        "<external-completion-events>unfinished"
    ));
    assert!(!is_external_notice("ordinary user message"));
    let source = "Goal context that must not be displayed\n\n<external-completion-events>\n后台命令：echo done\n状态：completed\n</external-completion-events>";
    assert!(is_external_notice(source));
    let full = notice(source, ToolCallDisplayMode::Full);
    assert!(full.contains("Background command completed"), "{full}");
    assert!(!full.contains("Goal context"), "{full}");
}

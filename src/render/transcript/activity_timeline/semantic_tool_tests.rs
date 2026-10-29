use super::super::test_support::options;
use super::*;
use crate::config::TuiMode;
use crate::render::{activity_animation::strip_ansi_for_test, tui_mode};

/// 【终端】【语义工具测试】读取纯文本，排除 ANSI 着色对断言的影响。
/// 参数: `document` 为工具分组文档；返回: 拼接后的可见文本。
fn text(document: &FullscreenDocument) -> String {
    strip_ansi_for_test(
        &document
            .lines
            .iter()
            .map(AnsiLine::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// 【终端】【待办展示测试】展开待办使用任务内容、状态与进度，不重复标题或展示内部 ID。
/// 参数: 无；返回: 无。
#[test]
fn todo_expansion_reuses_semantic_task_list() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_tool_call("todo".into(), r#"{"action":"list"}"#.into());
    store.push_tool_result("todo".into(), true, r#"{"ok":true,"items":[{"id":"todo_internal_1","text":"Inspect renderer","status":"completed"},{"id":"todo_internal_2","text":"Fix layout","status":"in_progress"},{"id":"todo_internal_3","text":"Verify tests","status":"pending"}]}"#.into());
    let plain =
        text(&store.render_fullscreen(100, &options(), &HashSet::from([ParagraphKey::whole(0)])));
    assert!(plain.contains("1/3"), "{plain}");
    for task in ["Inspect renderer", "Fix layout", "Verify tests"] {
        assert!(plain.contains(task), "{plain}");
    }
    assert!(
        plain.contains('✓') && plain.contains('→') && plain.contains('○'),
        "{plain}"
    );
    assert!(!plain.contains("\"items\""), "{plain}");
    assert!(!plain.contains("todo_internal"), "{plain}");
    assert_eq!(plain.matches("Todo").count(), 1, "{plain}");
    tui_mode::set_override(None);
}

/// 【终端】【待办摘要测试】收起的待办变更仍显示任务内容，不仅显示笼统动作。
/// 参数: 无；返回: 无。
#[test]
fn collapsed_todo_update_shows_changed_task() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_tool_call(
        "todo".into(),
        r#"{"action":"update","id":"internal","status":"completed"}"#.into(),
    );
    store.push_tool_result("todo".into(), true, r#"{"ok":true,"changed":[{"text":"Fix layout","status":"completed"}],"items":[{"text":"Fix layout","status":"completed"}]}"#.into());
    let plain = text(&store.render_fullscreen(100, &options(), &HashSet::new()));
    assert!(plain.contains("Fix layout"), "{plain}");
    assert!(!plain.contains("internal"), "{plain}");
    tui_mode::set_override(None);
}

/// 【终端】【通用工具测试】未知工具的结果也展示语义字段，不回退到 JSON 对象。
/// 参数: 无；返回: 无。
#[test]
fn custom_tool_details_are_readable_without_json() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_tool_call(
        "custom_tool".into(),
        r#"{"action":"inspect","options":{"limit":20}}"#.into(),
    );
    store.push_tool_result(
        "custom_tool".into(),
        true,
        r#"{"success":true,"records":[{"name":"renderer","status":"ready"}]}"#.into(),
    );
    let plain =
        text(&store.render_fullscreen(100, &options(), &HashSet::from([ParagraphKey::whole(0)])));
    assert!(plain.contains("Action: inspect"), "{plain}");
    assert!(plain.contains("Limit: 20"), "{plain}");
    assert!(plain.contains("Name: renderer"), "{plain}");
    assert!(!plain.contains("\"records\""), "{plain}");
    tui_mode::set_override(None);
}

/// 【终端】【子任务链路】实际执行链展开保留任务标题与结论，不展示快照协议。
/// 参数: 无；返回: 无。
#[test]
fn concise_subagent_transcript_keeps_conclusion() {
    let mut store = TranscriptStore::new(200);
    store.push_tool_call("subagent".into(), "{}".into());
    store.push_tool_result("subagent".into(), true, serde_json::json!({"ok":true,"subagent":{"id":"INTERNAL_ID","description":"Read-only search test","status":"completed","result":"**Conclusion**: two files found."}}).to_string());
    let plain = text(&store.render_fullscreen(100, &options(), &HashSet::new()));
    assert!(
        plain
            .lines()
            .next()
            .unwrap()
            .contains("Read-only search test"),
        "{plain}"
    );
    assert!(plain.contains("two files found"), "{plain}");
    assert!(
        !plain.contains("INTERNAL_ID") && !plain.contains("Success:"),
        "{plain}"
    );
}

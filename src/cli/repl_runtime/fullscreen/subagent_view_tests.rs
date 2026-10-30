use super::runtime_tests::{chrome, runtime};
use crate::cli::repl_runtime::ReplRuntime;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::ParagraphKey;
use crate::tools::subagent_state as state;

/// 【全屏测试】【正文读取】提取当前会话正文。
/// 参数: runtime 为全屏运行期；返回: 无终端样式的正文。
fn body(runtime: &ReplRuntime) -> String {
    runtime
        .fullscreen
        .as_ref()
        .unwrap()
        .state
        .document
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【全屏测试】【子会话切换】沿用全屏模式，段落展开与滚动状态不跨会话复用。
/// 参数: 无；返回: 无。
#[test]
fn subagent_switch_keeps_mode_and_resets_view_interactions() {
    let root = tempfile::tempdir().unwrap();
    let owner = root.path().to_string_lossy().into_owned();
    let (snapshot, _cancel) =
        state::create_subagent_for_owner(&owner, "child view".into(), "general".into(), 8);
    state::timeline_streaming_text(&snapshot.id, "CHILD-THOUGHT", true);
    state::timeline_tool_started(&snapshot.id, "run_command", r#"{"command":"printf CHILD"}"#);
    let mut runtime = runtime(1);
    runtime
        .transcript
        .push_tool_call("subagent".into(), r#"{"action":"start"}"#.into());
    runtime.transcript.push_tool_result(
        "subagent".into(),
        true,
        serde_json::json!({"subagent":{"id":snapshot.id,"status":"running"}}).to_string(),
    );
    runtime
        .update_composer(&chrome(), "draft", 5, false, Vec::new(), 0)
        .unwrap();
    runtime.enter_fullscreen().unwrap();
    let session = runtime.fullscreen.as_mut().unwrap();
    session.state.expanded.insert(ParagraphKey::whole(1));
    session.state.follow = false;
    session.pending_copy = Some("old selection".into());
    assert!(runtime.transcript.enter_subagent_view(3));
    runtime.paint_fullscreen().unwrap();
    let session = runtime.fullscreen.as_ref().unwrap();
    assert!(session.state.expanded.is_empty());
    assert!(session.state.follow);
    assert!(session.pending_copy.is_none());
    assert!(!session.state.document.activities.is_empty());
    assert!(body(&runtime).contains("CHILD-THOUGHT"));
    assert!(!body(&runtime).contains("question 0"));
    assert!(runtime.fullscreen_active());
    assert!(crate::render::tui_mode::prefers_fullscreen());
    assert!(runtime.transcript.exit_subagent_view());
    runtime.paint_fullscreen().unwrap();
    assert!(body(&runtime).contains("question 0"));
    assert!(!body(&runtime).contains("CHILD-THOUGHT"));
    assert!(runtime.fullscreen_active());
    state::clear_subagents_for_owner(&owner);
}

use super::*;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};
use crate::tools::subagent_state as state;

struct Fixture {
    root: tempfile::TempDir,
    id: String,
    store: TranscriptStore,
}

impl Fixture {
    /// 创建已切入子任务的会话；参数: 无；返回: 独立测试会话。
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let owner = root.path().to_string_lossy().into_owned();
        let (snapshot, _cancel) =
            state::create_subagent_for_owner(&owner, "render fixture".into(), "general".into(), 8);
        let mut store = TranscriptStore::new(500);
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "MAIN-ANSWER".into(),
        });
        store.finalize_live_tail();
        store.push_tool_call(
            "subagent".into(),
            serde_json::json!({"action":"start"}).to_string(),
        );
        store.push_tool_result(
            "subagent".into(),
            true,
            serde_json::json!({"subagent":{"id":snapshot.id,"status":"running"}}).to_string(),
        );
        assert!(store.enter_subagent_view(1));
        Self {
            root,
            id: snapshot.id,
            store,
        }
    }

    /// 写入思考、工具与正文；参数: 无；返回: 无。
    fn populate(&self) {
        state::timeline_streaming_text(&self.id, "PRIVATE-THOUGHT", true);
        state::timeline_tool_started(
            &self.id,
            "run_command",
            r#"{"command":"printf CHILD-COMMAND"}"#,
        );
        state::timeline_tool_finished(&self.id, "run_command", true, "CHILD-OUTPUT");
        state::timeline_streaming_text(&self.id, "CHILD-ANSWER", false);
    }
}

impl Drop for Fixture {
    /// 清理测试任务；参数: 无；返回: 无。
    fn drop(&mut self) {
        state::clear_subagents_for_owner(&self.root.path().to_string_lossy());
    }
}

/// 构造完整展示选项；参数: 无；返回: 主会话选项。
fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    }
}

/// 去除终端控制样式；参数: lines 为显示行；返回: 可断言文本。
fn plain(lines: &[super::super::AnsiLine]) -> String {
    lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【终端】【模式继承】子会话随主会话选择切换，全屏沿用执行链，主会话内容保持不变。
/// 参数: 无；返回: 无。
#[test]
fn subagent_follows_main_render_mode_and_returns_to_main() {
    let mut fixture = Fixture::new();
    fixture.populate();
    crate::render::tui_mode::configure(crate::config::TuiMode::Inline);
    let inline = fixture.store.display_window(80, &options(), 500, 0);
    let inline_text = plain(&inline.lines);
    assert!(inline_text.contains("CHILD-ANSWER"), "{inline_text}");
    assert!(!inline_text.contains("1 thought"), "{inline_text}");

    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let full = fixture.store.display_window(80, &options(), 500, 0);
    let full_text = plain(&full.lines);
    assert!(full_text.contains("1 thought"), "{full_text}");
    assert!(full_text.contains("CHILD-COMMAND"), "{full_text}");
    assert_eq!(
        crate::render::tui_mode::current(),
        crate::config::TuiMode::Fullscreen
    );

    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Inline));
    let inline_again = fixture.store.display_window(80, &options(), 500, 0);
    assert_eq!(plain(&inline_again.lines), inline_text);
    assert!(fixture.store.exit_subagent_view());
    let main = fixture.store.display_window(80, &options(), 500, 0);
    let main_text = plain(&main.lines);
    assert!(main_text.contains("MAIN-ANSWER"), "{main_text}");
    assert!(!main_text.contains("CHILD-ANSWER"), "{main_text}");
}

/// 【终端】【共享偏好】两种视图均遵循主会话的隐藏设置，正文不受影响。
/// 参数: 无；返回: 无。
#[test]
fn visibility_preferences_apply_in_both_modes() {
    let mut fixture = Fixture::new();
    fixture.populate();
    let hidden = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Hidden,
        tool_call_mode: ToolCallDisplayMode::Hidden,
    };
    for mode in [
        crate::config::TuiMode::Fullscreen,
        crate::config::TuiMode::Inline,
    ] {
        crate::render::tui_mode::set_override(Some(mode));
        let view = fixture.store.display_window(80, &hidden, 500, 0);
        let text = plain(&view.lines);
        assert!(text.contains("CHILD-ANSWER"), "{text}");
        assert!(!text.contains("PRIVATE-THOUGHT"), "{text}");
        assert!(!text.contains("CHILD-COMMAND"), "{text}");
        let visible = fixture.store.display_window(80, &options(), 500, 0);
        assert!(plain(&visible.lines).contains("PRIVATE-THOUGHT"));
    }
}

/// 【终端】【子会话折叠】全屏保留执行链命中范围，新增消息和结果刷新不丢失交互数据。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_document_keeps_activity_controls_and_message_anchors() {
    let mut fixture = Fixture::new();
    fixture.populate();
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let open = fixture
        .store
        .render_fullscreen(80, &options(), &HashSet::new());
    assert!(!open.activities.is_empty());
    let key = open
        .paragraphs
        .iter()
        .find(|span| span.key.part == super::super::ParagraphPart::ActivityGroup)
        .unwrap()
        .key;
    let folded = fixture
        .store
        .render_fullscreen(80, &options(), &HashSet::from([key]));
    assert!(folded
        .paragraphs
        .iter()
        .any(|span| span.key == key && !span.expanded));
    assert!(!plain(&folded.lines).contains("PRIVATE-THOUGHT"));
    assert!(plain(&folded.lines).contains("CHILD-ANSWER"));

    state::queue_subagent_message(&fixture.id, "user", "FOLLOW-UP").unwrap();
    let updated = fixture
        .store
        .render_fullscreen(80, &options(), &HashSet::new());
    assert_eq!(updated.anchors.len(), 1);
    assert!(updated.anchors[0].summary.contains("FOLLOW-UP"));
    assert!(plain(&updated.lines).contains("FOLLOW-UP"));
    assert!(!plain(&updated.lines).contains("Working"));
}

/// 【终端】【子会话状态】中断记录保持静态与中性，隐藏工具时不泄漏未完成调用。
/// 参数: 无；返回: 无。
#[test]
fn interrupted_and_result_only_subagents_preserve_state() {
    let mut fixture = Fixture::new();
    state::timeline_tool_started(&fixture.id, "read_file", r#"{"path":"UNFINISHED-FILE"}"#);
    state::finish_subagent(
        &fixture.id,
        "interrupted",
        Some("RESULT-ONLY".into()),
        Some("INTERRUPTION-REASON".into()),
        None,
    );
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let full = fixture
        .store
        .render_fullscreen(40, &options(), &HashSet::new());
    let text = plain(&full.lines);
    assert!(text.contains("RESULT-ONLY"), "{text}");
    assert!(text.contains("INTERRUPTION-REASON"), "{text}");
    assert!(!text.contains("Working"));
    assert!(!text.contains("Reading"));
    assert!(!full
        .lines
        .iter()
        .any(|line| line.as_str().contains("\x1b[31m")));
    let hidden = TranscriptRenderOptions {
        tool_call_mode: ToolCallDisplayMode::Hidden,
        ..options()
    };
    let view = fixture
        .store
        .render_fullscreen(40, &hidden, &HashSet::new());
    assert!(!plain(&view.lines).contains("UNFINISHED-FILE"));
}

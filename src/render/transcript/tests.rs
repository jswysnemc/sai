use super::line::AnsiLine;
use super::{TranscriptMode, TranscriptRenderOptions, TranscriptStore};
use crate::llm::{ChatStreamChunk, ChatStreamKind, ToolCallStreamProgress};
use crate::render::work_status::WorkStatus;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

fn chunk(kind: ChatStreamKind, text: &str) -> ChatStreamChunk {
    ChatStreamChunk {
        kind,
        text: text.to_string(),
    }
}

#[test]
fn ansi_lines_are_prewrapped_at_requested_width() {
    let lines = AnsiLine::wrap_block("\x1b[31mabcdef\x1b[0m", 3);

    assert_eq!(lines.len(), 2);
    assert!(lines[0].as_str().contains("abc"));
    assert!(lines[1].as_str().contains("def"));
    assert!(lines.iter().all(|line| line.as_str().ends_with("\x1b[0m")));
}

#[test]
fn terminal_image_protocols_are_not_split_by_text_width() {
    let kitty = "\x1b_Gf=100,a=T;abcdefghijklmnopqrstuvwxyz\x1b\\";
    let iterm = "\x1b]1337;File=inline=1:abcdefghijklmnopqrstuvwxyz\x07";

    let kitty_lines = AnsiLine::wrap_block(kitty, 4);
    let iterm_lines = AnsiLine::wrap_block(iterm, 4);

    assert_eq!(kitty_lines.len(), 1);
    assert_eq!(iterm_lines.len(), 1);
    assert!(kitty_lines[0].as_str().contains(kitty));
    assert!(iterm_lines[0].as_str().contains(iterm));
}

#[test]
fn live_tail_is_visible_before_consolidation_and_retained_afterward() {
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Yolo, "inspect resize".to_string());
    store.push_chunk(&chunk(ChatStreamKind::Content, "streamed answer\n"));

    assert_eq!(store.display_live_tail(80, &options()).len(), 1);
    assert!(store
        .display_tail(80, &options())
        .iter()
        .any(|line| line.as_str().contains("streamed answer")));

    assert!(store.finalize_live_tail());
    assert!(store.display_live_tail(80, &options()).is_empty());
    assert!(store
        .display_tail(80, &options())
        .iter()
        .any(|line| line.as_str().contains("streamed answer")));
}

#[test]
fn live_table_is_emitted_once_without_cursor_replacement_sequences() {
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "| Tool | Purpose |\n| --- | --- |\n| read_file | Read files |\n",
    ));

    let preview = store.display_live_tail(80, &options());
    let preview = preview.iter().map(|line| line.as_str()).collect::<String>();
    assert!(preview.contains("| Tool | Purpose |"));
    assert!(!preview.contains('┌'));

    store.push_chunk(&chunk(ChatStreamKind::Content, "complete\n"));
    assert!(store.finalize_live_tail());
    let lines = store.display_tail(80, &options());
    let rendered = lines.iter().map(|line| line.as_str()).collect::<String>();

    assert!(rendered.contains('┌'));
    assert!(rendered.contains("read_file"));
    assert!(!rendered.contains("\x1b[1A"));
}

#[test]
fn live_reasoning_summary_animates_without_waiting_for_consolidation() {
    let mut store = TranscriptStore::new(100);
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, "inspect resize"));

    let first = store.display_live_tail(80, &options());
    assert!(store.advance_live_animation());
    let second = store.display_live_tail(80, &options());

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first, second);
    assert!(second[0].as_str().contains("14 chars"));
}

#[test]
fn live_tool_argument_preview_is_visible_until_the_call_is_finalized() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call_progress(&ToolCallStreamProgress {
        index: 0,
        name: Some("read_file".to_string()),
        arguments_chars: 12,
        arguments_bytes: 12,
        arguments_preview: r#"{"path":"REA"#.to_string(),
    });

    assert!(store
        .display_live_tail(80, &options())
        .iter()
        .any(|line| line.as_str().contains("Read")));

    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"README.md"}"#.to_string(),
    );
    assert!(store.display_live_tail(80, &options()).is_empty());
    assert!(store
        .display_tail(80, &options())
        .iter()
        .any(|line| line.as_str().contains("README.md")));
}

#[test]
fn work_status_is_replaced_without_becoming_history() {
    let mut store = TranscriptStore::new(100);

    assert!(store.set_work_status(WorkStatus::WaitingResponse));
    let waiting = store.display_live_tail(80, &options());
    assert!(waiting[0]
        .as_str()
        .contains(WorkStatus::WaitingResponse.label()));
    assert!(waiting[0].as_str().contains('·') || waiting[0].as_str().contains('s'));

    assert!(store.set_work_status(WorkStatus::Thinking));
    let thinking = store.display_live_tail(80, &options());
    assert!(thinking[0].as_str().contains(WorkStatus::Thinking.label()));
    assert!(!thinking[0]
        .as_str()
        .contains(WorkStatus::WaitingResponse.label()));

    assert!(store.advance_live_animation());
    let animated = store.display_live_tail(80, &options());
    assert!(animated[0].as_str().contains(WorkStatus::Thinking.label()));

    assert!(store.clear_work_status());
    assert!(store.display_live_tail(80, &options()).is_empty());
}

#[test]
fn work_status_hidden_when_live_reasoning_exists() {
    use crate::llm::{ChatStreamChunk, ChatStreamKind};

    let mut store = TranscriptStore::new(100);
    assert!(store.set_work_status(WorkStatus::Thinking));
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: "inspect plan".to_string(),
    });

    let live = store.display_live_tail(80, &options());
    let joined = live.iter().map(|line| line.as_str()).collect::<String>();
    assert!(!joined.contains(WorkStatus::Working.label()));
    assert!(joined.contains("thinking"));
}

#[test]
fn tool_progress_and_result_update_one_lifecycle_cell() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"README.md"}"#.to_string(),
    );
    store.push_tool_progress("read_file".to_string(), "reading".to_string());
    store.push_tool_result("read_file".to_string(), true, "contents".to_string());

    let rendered = store
        .display_tail(
            100,
            &TranscriptRenderOptions {
                reasoning_mode: ReasoningDisplayMode::Full,
                tool_call_mode: ToolCallDisplayMode::Full,
            },
        )
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert_eq!(rendered.matches("args:").count(), 1);
    assert_eq!(rendered.matches("output:").count(), 1);
    assert!(rendered.contains("reading"));
    assert!(rendered.contains("contents"));
}

#[test]
fn user_echo_uses_a_prominent_bullet() {
    let mut store = TranscriptStore::new(100);
    store.push_user_echo(TranscriptMode::Yolo, "inspect resize".to_string());

    assert!(store
        .display_tail(80, &options())
        .iter()
        .any(|line| line.as_str().contains("●")));
}

/// 验证自动输入回显使用蓝色圆点。
#[test]
fn automatic_echo_uses_a_blue_bullet() {
    let mut store = TranscriptStore::new(100);
    store.push_automatic_echo("后台任务已完成".to_string());

    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(rendered.contains("\x1b[38;5;39m●"));
}

#[test]
fn summary_mode_keeps_compact_tool_call_block_visible() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"README.md"}"#.to_string(),
    );

    let lines = store.display_tail(80, &options());

    assert!(!lines.is_empty());
    assert!(lines.iter().any(|line| line.as_str().contains("Read")));
}

#[test]
fn summary_mode_keeps_tool_progress_message_visible() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_progress(
        "subagent".to_string(),
        "subagent is checking the implementation".to_string(),
    );

    let lines = store.display_tail(80, &options());

    assert!(lines
        .iter()
        .any(|line| line.as_str().contains("subagent is checking")));
}

#[test]
fn row_cap_trims_prewrapped_rows_not_source_cells() {
    let mut store = TranscriptStore::new(2);
    store.push_meta("first".to_string());
    store.push_meta("second".to_string());
    store.push_meta("third".to_string());

    let lines = store.display_tail(80, &options());

    assert_eq!(lines.len(), 2);
    assert!(lines[0].as_str().contains("second"));
    assert!(lines[1].as_str().contains("third"));
}

/// 验证权限交互附着在既有命令视图并保留最终决定。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn permission_audit_stays_inside_existing_command_view() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "run_command".to_string(),
        r#"{"command":"cargo test","cwd":"/workspace"}"#.to_string(),
    );
    store.push_permission_request(crate::permission::PermissionRequest {
        id: "permission".to_string(),
        session_id: "session".to_string(),
        tool: "run_command".to_string(),
        arguments: r#"{"command":"cargo test","cwd":"/workspace"}"#.to_string(),
    });
    let pending = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(pending.contains("❯"));
    assert!(pending.contains(crate::i18n::text("Allow once", "允许一次")));
    assert!(!pending.contains(crate::i18n::text("Allowed once", "已允许一次")));
    assert!(store.set_permission_reply_draft("permission", Some("请改为只读检查".to_string())));
    let reply = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(reply.contains("请改为只读检查"));
    assert!(reply.contains(crate::i18n::text("Enter submit", "Enter 提交")));
    assert!(store.resolve_permission("permission", crate::permission::PermissionDecision::Allow));

    let rendered = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert!(rendered.contains("cargo"));
    assert!(rendered.contains("test"));
    assert!(rendered.contains(crate::i18n::text("Allowed once", "已允许一次")));
    assert!(!rendered.contains(r#"{"command""#));
    assert!(!rendered.contains(crate::i18n::text("Permission required", "需要权限确认")));
}

/// 验证 edit_file 权限选择直接附着在 diff 视图下方。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn permission_audit_stays_inside_existing_diff_view() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("audit.txt");
    std::fs::write(&path, "old\n").unwrap();
    let arguments = serde_json::json!({
        "path": path.to_string_lossy(),
        "content": "new\n"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);
    store.push_tool_call("edit_file".to_string(), arguments.clone());
    store.push_permission_request(crate::permission::PermissionRequest {
        id: "permission".to_string(),
        session_id: "session".to_string(),
        tool: "edit_file".to_string(),
        arguments,
    });

    let rendered = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert!(rendered.contains("old"));
    assert!(rendered.contains("new"));
    assert!(rendered.contains(crate::i18n::text("Allow once", "允许一次")));
    assert!(!rendered.contains(crate::i18n::text("Permission required", "需要权限确认")));
}

#[test]
fn diff_fill_is_reapplied_to_each_prewrapped_row() {
    let lines = AnsiLine::wrap_block("\x1b[48;5;22mabcdef\x1b[K\x1b[0m", 3);

    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|line| line.as_str().contains("\x1b[K")));
}

#[test]
fn diff_cell_keeps_pre_edit_snapshot_after_file_changes() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("snapshot.txt");
    std::fs::write(&path, "old\n").unwrap();
    let arguments = serde_json::json!({
        "path": path.to_string_lossy(),
        "content": "new\n"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);

    store.push_tool_call("edit_file".to_string(), arguments);
    std::fs::write(&path, "new\n").unwrap();
    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert!(rendered.contains("old"));
    assert!(rendered.contains("new"));
}

#[test]
fn background_subagent_cell_reads_persisted_timeline() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "检查项目".to_string(),
        "explore".to_string(),
        5,
    );
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"检查项目"}"#.to_string(),
    );
    store.push_tool_result(
        "subagent".to_string(),
        true,
        serde_json::json!({"ok":true,"subagent":subagent.clone()}).to_string(),
    );
    crate::tools::subagent_state::timeline_streaming_text(&subagent.id, "正在检查", true);

    assert!(store.has_running_subagents());
    let running_signature = store.subagent_signature();
    let rendered = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(rendered.contains("检查项目"));

    crate::tools::subagent_state::finish_subagent(
        &subagent.id,
        "completed",
        Some("检查完成".to_string()),
        None,
        None,
    );
    assert!(!store.has_running_subagents());
    assert_ne!(store.subagent_signature(), running_signature);
}

#[test]
fn diff_fill_reapplies_background_before_el() {
    // EL 必须在 reset 之前，背景才能铺满整行
    let lines = AnsiLine::wrap_block(
        "\x1b[48;5;22m\x1b[38;5;108mabcdef\x1b[48;5;22m\x1b[K\x1b[0m",
        80,
    );
    assert_eq!(lines.len(), 1);
    let s = lines[0].as_str();
    let k = s.find("\x1b[K").expect("el");
    let reset_after = s[k..].find("\x1b[0m");
    assert!(reset_after.is_some());
    // K 之前应仍有背景（48;5;22）
    assert!(s[..k].contains("48;5;22"));
}

#[test]
fn run_command_success_keeps_growing_output_in_summary() {
    use crate::render::tool_view::{self, ToolView};
    use crate::render::ToolCallDisplayMode;

    let mut view = ToolView::running(
        "run_command".to_string(),
        r#"{"command":"echo hi"}"#.to_string(),
    );
    let before = tool_view::render(&view, ToolCallDisplayMode::Summary);
    view.finish(
        true,
        r#"{"success":true,"exit_code":0,"stdout":"hi\n","stderr":""}"#.to_string(),
    );
    let after = tool_view::render(&view, ToolCallDisplayMode::Summary);
    assert!(
        !after.is_empty(),
        "success should not swallow the command view"
    );
    assert!(
        after.len() >= before.len(),
        "result should not shrink the view"
    );
    assert!(after.contains("hi") || after.contains("output") || after.contains("echo"));
}

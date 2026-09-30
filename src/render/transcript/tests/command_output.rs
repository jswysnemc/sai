use super::*;

#[test]
fn command_output_updates_live_cell_and_toggles_expansion() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "run_command".to_string(),
        r#"{"command":"test"}"#.to_string(),
    );
    let chunk = crate::tools::command::CommandOutputChunk {
        stream: crate::tools::command::CommandOutputStream::Stdout,
        bytes: b"one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n"
            .to_vec(),
        omitted_bytes: 0,
    };
    assert!(store.push_command_output("run_command", &chunk));
    let collapsed = store
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    // 前 2 后 4：可见 one/two 与 nine..twelve
    assert!(collapsed.contains("one"));
    assert!(collapsed.contains("two") || collapsed.contains("twelve"));
    assert!(collapsed.contains("twelve"));
    assert!(!collapsed.contains("five") || collapsed.contains("…"));
    assert!(collapsed.contains("…") || collapsed.contains("lines"));
    assert!(collapsed.contains("Ctrl+O"));

    assert!(store.toggle_latest_command_output());
    let expanded = store
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(expanded.contains("six"));
    assert!(expanded.contains("seven"));
    assert!(!expanded.contains("Ctrl+O"));

    store.push_tool_result(
        "run_command".to_string(),
        true,
        serde_json::json!({
            "success": true,
            "exit_code": 0,
            "stdout": "final result was truncated",
            "stderr": ""
        })
        .to_string(),
    );
    let completed_expanded = store
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(completed_expanded.contains("six"));
    assert!(completed_expanded.contains("twelve"));
    assert!(!completed_expanded.contains("Ctrl+O"));

    assert!(store.toggle_latest_command_output());
    let completed_collapsed = store
        .display_tail(120, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(completed_collapsed.contains("one"));
    assert!(completed_collapsed.contains("twelve"));
    assert!(completed_collapsed.contains("Ctrl+O"));
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
        auto_audit: false,
        sandbox: None,
    });
    let pending = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(pending.contains("❯"));
    assert!(pending.contains("Allow once"));
    assert!(!pending.contains("Allowed once"));
    assert!(store.set_permission_reply_draft("permission", Some("请改为只读检查".to_string())));
    let reply = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(reply.contains("请改为只读检查"));
    assert!(reply.contains("Enter submit"));
    assert!(store.resolve_permission(
        "permission",
        crate::permission::PermissionDecision::allow_once()
    ));

    let rendered = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert!(rendered.contains("cargo"));
    assert!(rendered.contains("test"));
    assert!(rendered.contains("Allowed once"));
    assert!(!rendered.contains(r#"{"command""#));
    assert!(!rendered.contains("Permission required"));
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

/// 验证审批卡在决定前后都展示沙箱范围，命令结果展示沙箱拦截说明。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn sandbox_scope_and_denial_render_in_command_view() {
    let mut store = TranscriptStore::new(100);
    let arguments = r#"{"command":"curl https://x.dev","justification":"fetch schema"}"#;
    store.push_tool_call("run_command".to_string(), arguments.to_string());
    store.push_permission_request(crate::permission::PermissionRequest {
        id: "permission".to_string(),
        session_id: "session".to_string(),
        tool: "run_command".to_string(),
        arguments: arguments.to_string(),
        auto_audit: false,
        sandbox: Some(crate::permission::SandboxScope {
            kind: crate::permission::SandboxScopeKind::Escalated,
            backend: "bwrap".to_string(),
            network: false,
            reasons: vec!["network".to_string()],
            justification: Some("fetch schema".to_string()),
        }),
    });
    let text = |store: &mut TranscriptStore| {
        store
            .display_tail(100, &options())
            .iter()
            .map(|line| line.as_str())
            .collect::<String>()
    };
    let pending = text(&mut store);
    assert!(pending.contains("Leaves the sandbox") || pending.contains("批准后在沙箱外执行"));
    assert!(pending.contains("fetch schema"));
    assert!(store.resolve_permission(
        "permission",
        crate::permission::PermissionDecision::allow_once()
    ));
    assert!(text(&mut store).contains("fetch schema"));

    store.push_tool_result(
        "run_command".to_string(),
        false,
        r#"{"mode":"foreground","completed":true,"success":false,"exit_code":6,"stdout":"","stderr":"curl: (6) Could not resolve host","sandbox_denial":{"kind":"network","evidence":"curl","hint":"rerun escalated"}}"#.to_string(),
    );
    let finished = text(&mut store);
    assert!(finished.contains("Blocked by sandbox") || finished.contains("沙箱拦截"));
    assert!(finished.contains("rerun escalated"));
}

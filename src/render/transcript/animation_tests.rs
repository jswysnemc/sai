use super::test_support::options;
use super::TranscriptStore;
use crate::render::activity_animation::strip_ansi_for_test;

/// 【终端】【动效测试】工具运行时引导符呼吸且不改变布局，完成后保持静态。
/// 参数：无；返回：无。
#[test]
fn tool_guides_pulse_only_while_running() {
    for (name, args, output) in [
        (
            "run_command",
            r#"{"command":"cargo test"}"#,
            r#"{"success":true,"exit_code":0,"stdout":"done","stderr":""}"#,
        ),
        ("web_search", r#"{"query":"Rust"}"#, "search complete"),
    ] {
        let mut store = TranscriptStore::new(100);
        store.push_tool_call(name.into(), args.into());
        let first = store.cells[0].display_lines_framed(80, &options(), 4);
        let later = store.cells[0].display_lines_framed(80, &options(), 12);
        let first_title = strip_ansi_for_test(first[0].as_str());
        let later_title = strip_ansi_for_test(later[0].as_str());
        assert!(first_title.starts_with('▮'), "{first_title}");
        assert_eq!(first_title, later_title);
        assert_ne!(first[0], later[0]);
        assert_eq!(first.len(), later.len());

        store.push_tool_result(name.into(), true, output.into());
        assert_eq!(
            store.cells[0].display_lines_framed(80, &options(), 4),
            store.cells[0].display_lines_framed(80, &options(), 12),
        );
    }
}

/// 后台进程在启动调用返回后仍逐帧渲染，结束后恢复静态缓存。
#[test]
fn background_guide_keeps_animating_through_render_cache() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "background_command".into(),
        r#"{"action":"start","command":"build --watch"}"#.into(),
    );
    store.push_tool_result(
        "background_command".into(),
        true,
        serde_json::json!({
            "task": {"id": "animated-command", "command": "build --watch", "status": "running"},
            "stdout": "watching files"
        })
        .to_string(),
    );
    let mut cache = super::render_cache::RenderCache::default();
    let first = cache.lines_for(0, &store.cells[0], 80, &options(), 4);
    let later = cache.lines_for(0, &store.cells[0], 80, &options(), 12);
    assert_ne!(first[0], later[0], "后台命令引导符不应被定稿缓存冻结");
    assert_eq!(first.len(), later.len());
    assert_eq!(
        cache.count_for(0, &store.cells[0], 80, &options(), 12),
        later.len()
    );

    store.update_background_logs(
        "animated-command",
        &serde_json::json!({"task": {"status": "exited"}, "exit_code": 0}),
    );
    cache.invalidate(0);
    let finished = cache.lines_for(0, &store.cells[0], 80, &options(), 4);
    assert_eq!(
        finished,
        cache.lines_for(0, &store.cells[0], 80, &options(), 12)
    );
    assert!(strip_ansi_for_test(finished[0].as_str()).contains("$ build --watch"));
}

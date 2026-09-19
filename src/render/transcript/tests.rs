#[path = "tests/command_output.rs"]
mod command_output;
#[path = "tests/long_tables.rs"]
mod long_tables;
#[path = "tests/markdown_cache.rs"]
mod markdown_cache;
#[path = "tests/reasoning.rs"]
mod reasoning;
#[path = "tests/subagent_panels.rs"]
mod subagent_panels;
use super::line::AnsiLine;
use super::test_support::{chunk, options};
use super::{TranscriptMode, TranscriptRenderOptions, TranscriptStore};
use crate::llm::{ChatStreamKind, ToolCallStreamProgress};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

#[path = "tests/diff_wrapping.rs"]
mod diff_wrapping;
#[path = "tests/result_diff.rs"]
mod result_diff;

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

    // 区块前空行 + 正文一行
    assert_eq!(store.display_live_tail(80, &options()).len(), 2);
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

    // live 全量重绘：已确认表格按当前行集合输出边框表，不用光标回退
    let preview = store.display_live_tail(80, &options());
    let preview = preview.iter().map(|line| line.as_str()).collect::<String>();
    assert!(preview.contains('┌'));
    assert!(preview.contains("read_file"));
    assert!(!preview.contains("\x1b[1A"));

    store.push_chunk(&chunk(ChatStreamKind::Content, "complete\n"));
    assert!(store.finalize_live_tail());
    let lines = store.display_tail(80, &options());
    let rendered = lines.iter().map(|line| line.as_str()).collect::<String>();

    assert!(rendered.contains('┌'));
    assert!(rendered.contains("read_file"));
    assert!(!rendered.contains("\x1b[1A"));
}

#[test]
fn live_tool_argument_preview_is_visible_until_the_call_is_finalized() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call_progress(&ToolCallStreamProgress {
        edit_diff_counts: None,
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
fn streaming_content_grows_without_live_cap() {
    // 普通正文流式渲染稳定：必须完整进入窗口并随内容增长，
    // 不能被 live 上限困在固定高度内反复重绘
    let mut store = TranscriptStore::new(500);
    // 末行需带换行：流式渲染只输出完整行
    let body = (1..=60)
        .map(|n| format!("正文第 {n} 行\n"))
        .collect::<String>();
    store.push_chunk(&chunk(ChatStreamKind::Content, &body));

    let window = store.display_window_with_live_cap(80, &options(), 64, usize::MAX, 12);
    assert!(
        window.total >= 60,
        "稳定正文不应被截断到 live 上限: total={}",
        window.total
    );
    let rendered = window
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(rendered.contains("正文第 1 行"), "首行必须仍在窗口内");
    assert!(rendered.contains("正文第 60 行"));

    // 追加内容后总行数继续增长
    store.push_chunk(&chunk(ChatStreamKind::Content, "正文第 61 行\n"));
    let grown = store.display_window_with_live_cap(80, &options(), 64, usize::MAX, 12);
    assert!(grown.total > window.total, "追加正文后窗口总行数应增长");
}

#[test]
fn open_table_preview_grows_past_mutable_layout_budget() {
    // 【终端】【长表格】超过可变布局预算后只固定列宽，全部行仍参与输出
    let mut store = TranscriptStore::new(500);
    let mut source = String::from("| 列一 | 列二 |\n|---|---|\n");
    for n in 1..=40 {
        source.push_str(&format!("| 行{n} | 值{n} |\n"));
    }
    store.push_chunk(&chunk(ChatStreamKind::Content, &source));

    let window = store.display_window_with_live_cap(80, &options(), 64, usize::MAX, 12);
    assert!(
        window.total > 40,
        "开放表格应保留全部数据行: total={}",
        window.total
    );
}

/// 【终端】【Ctrl+O】定稿 diff 进入分页列表，内联展开会失效缓存。
#[test]
fn toggle_inline_expand_unfolds_finalized_diff() {
    let cwd = crate::runtime_cwd::current_dir().unwrap();
    let temp = tempfile::tempdir_in(cwd).unwrap();
    let path = temp.path().join("ctrl-o-diff.txt");
    let old: String = (1..=40).map(|n| format!("line{n}\n")).collect();
    let new: String = (1..=40).map(|n| format!("changed{n}\n")).collect();
    std::fs::write(&path, &old).unwrap();
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "old_string": old,
        "new_string": new
    })
    .to_string();
    let mut store = TranscriptStore::new(200);
    store.push_tool_call("str_replace".to_string(), arguments);
    let _ = store.display_tail(80, &options());

    let blocks = store.expandable_blocks();
    assert!(
        blocks
            .iter()
            .any(|block| block.kind == crate::render::transcript::ExpandableBlockKind::Diff),
        "diff must be in Ctrl+O pager"
    );

    assert!(store.toggle_inline_expand());
    let expanded = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(
        !expanded.contains("Ctrl+O"),
        "expanded diff should drop fold hint: {expanded}"
    );
}

#[test]
fn markdown_table_lines_fit_display_width() {
    // 表格布局必须使用与折行相同的宽度：任何超宽行都会被 wrap_block
    // 折成无缩进碎片，表现为重绘后的框线错乱
    let source = "| 框架 | 类型 | 首次发布 | 维护方 | 热度指数 |\n\
                  |---|---|---|---|---|\n\
                  | React | 组件化库 | 2013 | Meta | 5 |\n\
                  | Vue | 渐进式框架 | 2014 | 尤雨溪 / 社区 | 5 |\n\
                  | Angular | 全栈框架 | 2010 | Google | 4 |";
    let cell = super::cell::HistoryCell::markdown(source.to_string());
    for width in [40usize, 60, 81, 120] {
        let content_width = width
            .saturating_sub(crate::render::content_indent::CONTENT_LEFT_INDENT)
            .max(1);
        let lines = cell.display_lines(content_width, &options());
        for (index, line) in lines.iter().enumerate() {
            let plain = strip_ansi(line.as_str());
            let display_width: usize = plain
                .chars()
                .map(|ch| unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0))
                .sum();
            assert!(
                display_width <= width,
                "width={width} line {index} overflows: {plain:?}"
            );
        }
    }
}

/// 去掉 ANSI 序列便于宽度断言。
fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut escape = false;
    for ch in text.chars() {
        if ch == '\x1b' {
            escape = true;
            continue;
        }
        if escape {
            if ch.is_ascii_alphabetic() {
                escape = false;
            }
            continue;
        }
        out.push(ch);
    }
    out
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
fn concurrent_same_name_tools_update_in_place_fifo() {
    let mut store = TranscriptStore::new(100);
    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"a.rs","offset":1,"limit":20}"#.to_string(),
    );
    store.push_tool_call(
        "read_file".to_string(),
        r#"{"path":"b.rs","offset":10,"limit":40}"#.to_string(),
    );
    store.push_tool_call("read_file".to_string(), r#"{"path":"c.rs"}"#.to_string());

    store.push_tool_result("read_file".to_string(), true, "a".to_string());
    let mid = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .collect::<String>();
    assert!(mid.contains("Read a.rs:1–20"), "{mid}");
    assert!(mid.contains("Reading b.rs:10–49"), "{mid}");
    assert!(mid.contains("Reading c.rs"), "{mid}");
    assert_eq!(mid.matches("Read ok").count(), 0, "{mid}");

    store.push_tool_result("read_file".to_string(), true, "b".to_string());
    store.push_tool_result("read_file".to_string(), true, "c".to_string());
    let done = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .collect::<String>();
    assert!(done.contains("Read a.rs:1–20"), "{done}");
    assert!(done.contains("Read b.rs:10–49"), "{done}");
    assert!(done.contains("Read c.rs"), "{done}");
    assert!(!done.contains("Reading "), "{done}");
    assert_eq!(done.matches("Read ok").count(), 0, "{done}");
}

#[test]
fn compaction_started_updates_in_place_without_x0() {
    let mut store = TranscriptStore::new(100);
    store.push_compaction_started(0, "grok-4.6".to_string());
    let started = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .collect::<String>();
    assert!(started.contains("Compacting context"), "{started}");
    assert!(!started.contains("×0"), "{started}");

    store.push_compaction_finished(true, None, None, Some("notes".to_string()));
    let finished = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .collect::<String>();
    assert!(finished.contains("Compacted context"), "{finished}");
    assert!(!finished.contains("Compacting context"), "{finished}");
    assert_eq!(
        finished.matches("Compacted context").count(),
        1,
        "{finished}"
    );
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

/// 验证轮次总览渲染后自带按宽度绘制的 turn 分割线，普通提示没有。
#[test]
fn turn_summary_appends_a_width_fitted_rule() {
    let mut store = TranscriptStore::new(100);
    store.push_turn_summary("\x1b[2m•\x1b[0m \x1b[2mContext:\x1b[0m 8.0k / 1000k".to_string());

    let width = 48;
    let lines = store.display_tail(width, &options());
    let rule = lines
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
        .find(|plain| !plain.trim().is_empty() && plain.trim().chars().all(|ch| ch == '─'))
        .expect("turn summary must append a horizontal rule");
    // 分割线恰好占满正文净宽，terminal 缩放时由渲染层按新宽度重画
    assert_eq!(rule.trim().chars().count(), width);

    let mut plain_store = TranscriptStore::new(100);
    plain_store.push_meta("已切换模型".to_string());
    let has_rule = plain_store
        .display_tail(width, &options())
        .iter()
        .map(|line| crate::render::activity_animation::strip_ansi_for_test(line.as_str()))
        .any(|plain| !plain.trim().is_empty() && plain.trim().chars().all(|ch| ch == '─'));
    assert!(!has_rule, "plain notices must not carry a turn rule");
}

#[test]
fn row_cap_trims_prewrapped_rows_not_source_cells() {
    // meta 前有区块空行，单条占 2 行；cap=2 时只保留最新一条
    let mut store = TranscriptStore::new(2);
    store.push_meta("first".to_string());
    store.push_meta("second".to_string());
    store.push_meta("third".to_string());

    let lines = store.display_tail(80, &options());

    assert_eq!(lines.len(), 2);
    assert!(lines.iter().any(|line| line.as_str().contains("third")));
    assert!(!lines.iter().any(|line| line.as_str().contains("first")));
    assert!(!lines.iter().any(|line| line.as_str().contains("second")));
}

/// 验证编辑类权限选择附着在摘要行与 diff 正文下方（无旧式 Added 标题）。
#[test]
fn permission_audit_stays_inside_existing_diff_view() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("audit.txt");
    std::fs::write(&path, "old\n").unwrap();
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "old_string": "old",
        "new_string": "new"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);
    store.push_tool_call("str_replace".to_string(), arguments.clone());
    store.push_permission_request(crate::permission::PermissionRequest {
        id: "permission".to_string(),
        session_id: "session".to_string(),
        tool: "str_replace".to_string(),
        arguments: arguments,
        auto_audit: false,
    });

    let rendered = store
        .display_tail(100, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert!(rendered.contains("Replacing"), "{rendered}");
    assert!(rendered.contains("Allow once"), "{rendered}");
    assert!(!rendered.contains("Permission required"), "{rendered}");
    assert!(rendered.contains("old"), "{rendered}");
    assert!(rendered.contains("new"), "{rendered}");
    assert!(!rendered.contains("Added"), "{rendered}");
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
        "path": path.display().to_string(),
        "old_string": "old",
        "new_string": "new"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);

    store.push_tool_call("str_replace".to_string(), arguments);
    std::fs::write(&path, "new\n").unwrap();
    // Full 模式才展开冻结正文；写盘后仍应看到调用前的 old→new
    let full = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Summary,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    let rendered = store
        .display_tail(80, &full)
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();

    assert!(rendered.contains("old"), "{rendered}");
    assert!(rendered.contains("new"), "{rendered}");
    assert!(
        rendered.contains("Replacing") || rendered.contains("Replaced"),
        "{rendered}"
    );
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
fn markdown_hr_stays_inset_in_transcript_display() {
    let cell = super::cell::HistoryCell::markdown("before\n\n---\n\nafter\n".to_string());
    let width = 80usize;
    let content_width = width
        .saturating_sub(crate::render::content_indent::CONTENT_LEFT_INDENT)
        .max(1);
    let inset = crate::render::markdown_blocks::MARKDOWN_HR_SIDE_INSET
        .min(content_width.saturating_sub(1) / 2);
    let expected_dashes = content_width.saturating_sub(inset.saturating_mul(2)).max(1);
    let lines = cell.display_lines(content_width, &options());
    let hr = lines.iter().find_map(|line| {
        let plain = strip_ansi(line.as_str());
        plain.contains('─').then_some(plain)
    });
    let hr = hr.expect("markdown --- must render a horizontal rule");
    assert!(
        hr.starts_with("  ─"),
        "MD hr must sit past the guide column: {hr:?}"
    );
    assert!(
        !hr.starts_with('─'),
        "MD hr must not be flush-left like a turn rule: {hr:?}"
    );
    let dash_count = hr.chars().filter(|ch| *ch == '─').count();
    assert_eq!(
        dash_count, expected_dashes,
        "MD hr must inset both sides within the content column: {hr:?}"
    );
    assert!(
        hr.chars().count() < width,
        "MD hr must leave right inset vs terminal width: {hr:?}"
    );
}

#[test]
fn settled_write_file_shows_stat_line_not_run() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("notes.md");
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "content": "alpha\nbeta\n"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);
    store.push_tool_call("write_file".to_string(), arguments);
    store.push_tool_result(
        "write_file".to_string(),
        true,
        r#"{"changed_files":[{"path":"notes.md","added":2,"removed":0}]}"#.to_string(),
    );
    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    let plain = crate::render::activity_animation::strip_ansi_for_test(&rendered);
    let first = plain.lines().next().unwrap_or("");
    assert!(
        first.contains("Wrote") && first.contains("notes.md"),
        "{first}"
    );
    assert!(first.contains('+'), "{first}");
    assert!(!first.contains("run"), "{first}");
    assert!(plain.contains("alpha") || plain.contains("beta"), "{plain}");
}

#[test]
fn history_edit_file_restores_stat_line_and_diff_body() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("history.txt");
    std::fs::write(&path, "old\n").unwrap();
    let arguments = serde_json::json!({
        "path": path.display().to_string(),
        "old_string": "old",
        "new_string": "new"
    })
    .to_string();
    let mut store = TranscriptStore::new(100);
    store.push_history_tool_call("str_replace".to_string(), arguments);
    store.push_tool_result(
        "str_replace".to_string(),
        true,
        r#"{"changed_files":[{"path":"history.txt","added":1,"removed":1}]}"#.to_string(),
    );
    let rendered = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    // Summary 默认：Replaced +N -M 摘要行 + 冻结行级正文（无旧式 Added 标题）
    assert!(rendered.contains("Replaced"), "{rendered}");
    assert!(
        rendered.contains("+1") || rendered.contains('+'),
        "{rendered}"
    );
    assert!(rendered.contains("old"), "{rendered}");
    assert!(rendered.contains("new"), "{rendered}");
    assert!(!rendered.contains("Added"), "{rendered}");
}

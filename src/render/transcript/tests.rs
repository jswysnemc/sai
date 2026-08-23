use super::line::AnsiLine;
use super::test_support::{chunk, options};
use super::{TranscriptMode, TranscriptRenderOptions, TranscriptStore};
use crate::llm::{ChatStreamKind, ToolCallStreamProgress};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

#[path = "tests/diff_wrapping.rs"]
mod diff_wrapping;

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
fn reasoning_cell_lines_fit_display_width() {
    // 渲染宽度上下文注入后，thinking 正文折行必须与 display 宽度一致，
    // 不得产生被 wrap_block 二次折断的无缩进续行
    let source =
        "The user is asking \"你好,你能做什么\" - which means \"Hello, what can you do?\" \
                  in Chinese. This is a general question about my capabilities. Let me give a \
                  concise but helpful overview of what I can do.";
    let mut cell =
        crate::render::transcript::reasoning_cell::ReasoningCell::new(source.to_string());
    cell.expanded = true;
    let cell = super::cell::HistoryCell::Reasoning(cell);
    for width in [40usize, 60, 81, 100] {
        let lines = cell.display_lines(
            width,
            &TranscriptRenderOptions {
                reasoning_mode: ReasoningDisplayMode::Full,
                tool_call_mode: ToolCallDisplayMode::Summary,
            },
        );
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
            // 区块前空行与 ◦ 标题行跳过；正文必须带 gutter（`  └ ` 或四空格）
            if plain.is_empty() || plain.starts_with('◦') {
                continue;
            }
            assert!(
                plain.starts_with("  └ ") || plain.starts_with("    "),
                "width={width} line {index} lost gutter: {plain:?}"
            );
        }
    }
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
fn open_table_preview_stays_capped() {
    // 未闭合表格的列宽会回溯变化：其预览必须受 live 上限约束，
    // 避免中间帧被滚入 scrollback 成为残留
    let mut store = TranscriptStore::new(500);
    let mut source = String::from("| 列一 | 列二 |\n|---|---|\n");
    for n in 1..=40 {
        source.push_str(&format!("| 行{n} | 值{n} |\n"));
    }
    store.push_chunk(&chunk(ChatStreamKind::Content, &source));

    let window = store.display_window_with_live_cap(80, &options(), 64, usize::MAX, 12);
    assert!(
        window.total <= 12,
        "开放表格预览应截断到 live 上限: total={}",
        window.total
    );
}

#[test]
fn expanded_render_context_unfolds_reasoning() {
    // 备用屏回看：展开渲染上下文下折叠的思考正文全量输出，且不污染主屏缓存
    let mut store = TranscriptStore::new(200);
    let source = (1..=12)
        .map(|n| format!("thinking line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, &source));
    store.finalize_live_tail();

    let folded = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(!folded.contains("thinking line 6"), "默认应折叠中段");

    let expanded = crate::render::render_expand::with_expanded_render(|| {
        store
            .display_tail(80, &options())
            .iter()
            .map(|line| line.as_str())
            .collect::<String>()
    });
    assert!(expanded.contains("thinking line 6"));
    assert!(!expanded.contains("Ctrl+O"));

    // 退出展开上下文后主屏仍是折叠渲染（缓存未被展开结果污染）
    let folded_again = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(!folded_again.contains("thinking line 6"));
}

/// 【终端】【Ctrl+O】定稿思考可用内联展开，并失效渲染缓存。
#[test]
fn toggle_inline_expand_unfolds_finalized_reasoning() {
    let mut store = TranscriptStore::new(200);
    let source = (1..=12)
        .map(|n| format!("thinking line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    store.push_chunk(&chunk(ChatStreamKind::Reasoning, &source));
    store.finalize_live_tail();
    let _ = store.display_tail(80, &options());

    assert!(store.toggle_inline_expand());
    let expanded = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(expanded.contains("thinking line 6"));
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
fn subagent_view_switch_replaces_display_window() {
    let mut store = TranscriptStore::new(100);
    store.push_meta("主会话内容".to_string());
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"检查项目"}"#.to_string(),
    );
    // 绑定后台 ID：running 状态下 finish 只记录 ID
    store.push_tool_result(
        "subagent".to_string(),
        true,
        r#"{"subagent":{"id":"sub-test-1","status":"running"}}"#.to_string(),
    );

    // 1. 进入子智能体视图：窗口内容替换为其会话时间线
    assert!(store.enter_subagent_view(1));
    assert_eq!(store.viewing_subagent_id(), Some("sub-test-1"));
    let view = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    // 子智能体视图标题与主视图工具行同语汇（Delegating/Delegated）
    assert!(view.contains("Delegating") || view.contains("Delegated"));
    assert!(!view.contains("主会话内容"));

    // 2. 返回主视图：恢复主会话内容
    assert!(store.exit_subagent_view());
    assert!(!store.exit_subagent_view());
    let main = store
        .display_tail(80, &options())
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(main.contains("主会话内容"));
}

#[test]
fn subagent_overview_lists_running_or_viewing_only() {
    let mut store = TranscriptStore::new(100);
    // 已结束且未在查看的子智能体不出现在面板
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"完成的"}"#.to_string(),
    );
    store.push_tool_result("subagent".to_string(), true, "plain result".to_string());
    assert!(store.subagent_overview().is_empty());
}

/// 【终端】【agent 面板】同一子智能体的多次工具调用只保留一个面板条目。
///
/// 主代理每次 subagent 调用（start / wait / send / result）都会产生
/// 一个 transcript cell，此前每个 cell 都各占一行导致面板大量重复。
#[test]
fn subagent_overview_deduplicates_repeated_calls_by_id() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "诗歌文本多阶段分析".to_string(),
        "explore".to_string(),
        3,
    );
    let bound_result = format!(
        r#"{{"subagent":{{"id":"{}","status":"running"}}}}"#,
        subagent.id
    );
    let mut store = TranscriptStore::new(100);
    // 同一个子智能体：start + 多次 wait/send，每次调用都是一个独立 cell
    for _ in 0..3 {
        store.push_tool_call(
            "subagent".to_string(),
            format!(r#"{{"action":"wait","id":"{}"}}"#, subagent.id),
        );
        store.push_tool_result("subagent".to_string(), true, bound_result.clone());
    }

    let overview = store.subagent_overview();

    assert_eq!(overview.len(), 1, "同一子智能体必须去重: {overview:?}");
    assert!(overview[0].running);
    assert_eq!(overview[0].status, "run");
}

/// 【终端】【agent 面板】尚未返回的 wait 调用按参数中的 subagent_id 归并。
///
/// 回归：子智能体 ID 此前只从工具输出解析，进行中的调用还没有输出，于是
/// 拿不到 ID 也就无法去重。并发等待四个子智能体时，四条 wait 会各占一行，
/// 和真正的委派条目混在一起。
#[test]
fn subagent_overview_merges_pending_waits_by_argument_id() {
    let (subagent, _cancel) = crate::tools::subagent_state::create_subagent(
        "并发等待".to_string(),
        "general".to_string(),
        3,
    );
    let mut store = TranscriptStore::new(100);
    // 1. start 已返回并绑定后台 ID
    store.push_tool_call(
        "subagent".to_string(),
        r#"{"description":"并发等待"}"#.to_string(),
    );
    store.push_tool_result(
        "subagent".to_string(),
        true,
        format!(
            r#"{{"subagent":{{"id":"{}","status":"running"}}}}"#,
            subagent.id
        ),
    );
    // 2. 三次仍在进行中的 wait，没有任何结果可供解析
    for _ in 0..3 {
        store.push_tool_call(
            "subagent".to_string(),
            format!(r#"{{"action":"wait","subagent_id":"{}"}}"#, subagent.id),
        );
    }

    let overview = store.subagent_overview();

    assert_eq!(
        overview.len(),
        1,
        "进行中的 wait 应归并到同一条目: {overview:?}"
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
    assert!(mid.contains("Read a.rs:1+20"), "{mid}");
    assert!(mid.contains("Reading b.rs:10+40"), "{mid}");
    assert!(mid.contains("Reading c.rs"), "{mid}");
    assert_eq!(mid.matches("Read ok").count(), 0, "{mid}");

    store.push_tool_result("read_file".to_string(), true, "b".to_string());
    store.push_tool_result("read_file".to_string(), true, "c".to_string());
    let done = store
        .display_tail(80, &options())
        .iter()
        .map(|line| strip_ansi(line.as_str()))
        .collect::<String>();
    assert!(done.contains("Read a.rs:1+20"), "{done}");
    assert!(done.contains("Read b.rs:10+40"), "{done}");
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

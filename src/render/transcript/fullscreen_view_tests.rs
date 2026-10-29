use super::*;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::TranscriptMode;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 测试用渲染选项：思考按摘要折叠，与默认前台一致。
fn options() -> TranscriptRenderOptions {
    TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Summary,
        tool_call_mode: ToolCallDisplayMode::Summary,
    }
}

/// 构造两轮对话：每轮一条用户消息、一段长思考、一段回复。
fn store() -> TranscriptStore {
    let mut store = TranscriptStore::new(10_000);
    for turn in ["first question", "second question"] {
        store.push_user_echo(TranscriptMode::Yolo, turn.to_string());
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: (0..20).map(|i| format!("{turn}-think-{i}\n")).collect(),
        });
        store.finalize_live_tail();
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: format!("{turn} answer"),
        });
        store.finalize_live_tail();
    }
    store
}

/// 文档纯文本，便于断言。
fn plain(document: &FullscreenDocument) -> Vec<String> {
    document
        .lines
        .iter()
        .map(|line| strip_ansi_for_test(line.as_str()))
        .collect()
}

/// 用户消息锚点指向消息所在行，摘要取原文。
#[test]
fn anchors_point_at_user_messages() {
    let mut store = store();
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let text = plain(&document);
    assert_eq!(document.anchors.len(), 2);
    for (anchor, expected) in document
        .anchors
        .iter()
        .zip(["first question", "second question"])
    {
        assert_eq!(anchor.summary, expected);
        assert!(
            text[anchor.row].contains(expected),
            "{:?}",
            text[anchor.row]
        );
    }
    let second = document.anchors[1].row;
    assert_eq!(document.anchor_index_at(second), Some(1));
    assert_eq!(document.anchor_index_at(second - 1), Some(0));
}

/// 每个段落独立展开：展开第一段不影响第二段，行范围随之变化。
#[test]
fn paragraphs_expand_independently() {
    let mut store = store();
    let collapsed = store.render_fullscreen(
        80,
        &options(),
        &HashSet::from([
            ParagraphKey::activity(1, false),
            ParagraphKey::activity(4, false),
        ]),
    );
    let keys = collapsed
        .paragraphs
        .iter()
        .map(|span| span.key)
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 2, "两段思考都应可折叠");
    assert!(collapsed.paragraphs.iter().all(|span| !span.expanded));
    let joined = plain(&collapsed).join("\n");
    assert!(!joined.contains("first question-think-10"), "{joined}");

    let expanded = store.render_fullscreen(
        80,
        &options(),
        &HashSet::from([ParagraphKey::whole(1), ParagraphKey::activity(4, false)]),
    );
    let joined = plain(&expanded).join("\n");
    assert!(joined.contains("first question-think-10"), "{joined}");
    assert!(!joined.contains("second question-think-10"), "{joined}");
    assert!(expanded.paragraphs[0].expanded && !expanded.paragraphs[1].expanded);
    assert!(expanded.lines.len() > collapsed.lines.len());
    // 展开后后续锚点下移，点击命中仍准确
    assert!(expanded.anchors[1].row > collapsed.anchors[1].row);
    let span = &expanded.paragraphs[1];
    assert_eq!(
        expanded.paragraph_at(span.start).map(|s| s.key),
        Some(span.key)
    );
}

/// 流式思考尾部也能展开收起。
#[test]
fn live_reasoning_is_a_paragraph() {
    let mut store = TranscriptStore::new(10_000);
    store.push_user_echo(TranscriptMode::Yolo, "stream".into());
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: (0..20).map(|i| format!("live-{i}\n")).collect(),
    });
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    assert_eq!(
        document.paragraphs.last().map(|span| span.key),
        Some(ParagraphKey::activity(1, false))
    );
    let open = store.render_fullscreen(80, &options(), &HashSet::from([ParagraphKey::whole(1)]));
    assert!(plain(&open).join("\n").contains("live-10"));
}

/// 长消息摘要折叠空白并截断。
#[test]
fn summary_is_single_line_and_bounded() {
    let long = format!("a\n  b {}", "x".repeat(300));
    let summary = summarize(&long);
    assert!(summary.starts_with("a b "));
    assert!(summary.ends_with('…'));
    assert_eq!(summary.chars().count(), SUMMARY_CHARS + 1);
}

/// 构造一条命令很长、输出也很长的 run_command 卡片。
fn command_store() -> TranscriptStore {
    let mut store = TranscriptStore::new(1_000);
    let command = (0..12)
        .map(|index| format!("echo step-{index}"))
        .collect::<Vec<_>>()
        .join(" && \\\n");
    store.push_tool_call(
        "run_command".to_string(),
        serde_json::json!({ "command": command }).to_string(),
    );
    let stdout = (0..30)
        .map(|index| format!("out-{index}\n"))
        .collect::<String>();
    store.push_tool_result(
        "run_command".to_string(),
        true,
        serde_json::json!({ "success": true, "exit_code": 0, "stdout": stdout, "stderr": "" })
            .to_string(),
    );
    store
}

/// 【终端】【命令折叠】命令与输出共用一个标题控件，内部边界标记不进入正文。
/// 参数: 无；返回: 无。
#[test]
fn command_card_has_one_header_control() {
    let mut store = command_store();
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    assert_eq!(document.paragraphs.len(), 1);
    let span = &document.paragraphs[0];
    assert_eq!(span.control, span.start);
    let text = plain(&document).join("\n");
    assert!(text.contains("step-6") && text.contains("out-15"), "{text}");
}

/// 【终端】【命令切换】收起整条命令后只保留标题，再次展开恢复命令与输出。
/// 参数: 无；返回: 无。
#[test]
fn command_and_output_toggle_together() {
    let mut store = command_store();
    let collapsed = store.render_fullscreen(
        80,
        &options(),
        &HashSet::from([ParagraphKey::activity(0, false)]),
    );
    let text = plain(&collapsed).join("\n");
    assert!(
        !text.contains("step-6") && !text.contains("out-15"),
        "{text}"
    );
    assert!(!collapsed.paragraphs[0].expanded);
    let open = store.render_fullscreen(80, &options(), &HashSet::new());
    let text = plain(&open).join("\n");
    assert!(text.contains("step-6") && text.contains("out-15"), "{text}");
    assert!(open.paragraphs[0].expanded);
}

/// 【终端】【短命令标题】短命令也通过首行统一控制输出，不增加第二个控件。
/// 参数: 无；返回: 无。
#[test]
fn short_command_has_one_header_control() {
    let mut store = TranscriptStore::new(1_000);
    store.push_shell(
        "ls".into(),
        (0..30).map(|index| format!("file-{index}\n")).collect(),
        Some(0),
    );
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    assert_eq!(document.paragraphs.len(), 1);
    assert_eq!(document.paragraphs[0].control, document.paragraphs[0].start);
}

/// 【全屏代码】【展开回归】文件阅读卡片展开后仍保留关键字与字符串着色；无参数或返回值。
#[test]
fn expanded_read_file_keeps_syntax_colors() {
    let mut store = TranscriptStore::new(1000);
    store.push_tool_call("read_file".into(), r#"{"path":"example.rs"}"#.into());
    store.push_tool_result(
        "read_file".into(),
        true,
        "1\tfn main() {\n2\t    println!(\"hello\");\n3\t}".into(),
    );
    let collapsed = store.render_fullscreen(80, &options(), &HashSet::new());
    let key = ParagraphKey::whole(collapsed.paragraphs.last().unwrap().key.cell);
    let expanded = store.render_fullscreen(80, &options(), &HashSet::from([key]));
    let ansi = expanded
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(ansi.contains(crate::render::style::CODE_KEYWORD_STYLE));
    assert!(ansi.contains(crate::render::style::CODE_STRING_STYLE));
    assert!(plain(&expanded).join("\n").contains("println!"));
}

/// 【全屏代码】【思考回归】展开带代码围栏的思考内容时使用同一高亮链路；无参数或返回值。
#[test]
fn expanded_reasoning_keeps_fenced_code_colors() {
    let mut store = TranscriptStore::new(1000);
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: "说明\n\n```rust\nfn main() { println!(\"hello world\"); }\n```".into(),
    });
    store.finalize_live_tail();
    let collapsed = store.render_fullscreen(24, &options(), &HashSet::new());
    let key = ParagraphKey::whole(collapsed.paragraphs[0].key.cell);
    let expanded = store.render_fullscreen(24, &options(), &HashSet::from([key]));
    let ansi = expanded
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(ansi.contains(crate::render::style::CODE_KEYWORD_STYLE));
    assert!(ansi.contains(crate::render::style::CODE_STRING_STYLE));
    assert!(!plain(&expanded).join("\n").contains("```"));
}

/// 【提问展示】【全屏回归】实际展开路径保留问题和选项答案，移除重复说明，不显示原始协议；无参数和返回值。
#[test]
fn expanded_question_uses_readable_answers() {
    for width in [40, 80] {
        let mut store = TranscriptStore::new(1000);
        store.push_tool_call("ask_question".into(), r#"{"questions":[{"header":"Scope","question":"Select a module","options":[{"label":"UI","description":"Option details"}]}]}"#.into());
        store.push_tool_result("ask_question".into(), true, r#"{"status":"answered","answers":[{"header":"Scope","question":"Select a module","answer":"UI"}]}"#.into());
        let collapsed = store.render_fullscreen(width, &options(), &HashSet::new());
        let key = ParagraphKey::whole(collapsed.paragraphs.last().unwrap().key.cell);
        let expanded = store.render_fullscreen(width, &options(), &HashSet::from([key]));
        let text = plain(&expanded).join("\n");
        assert!(
            text.contains("Asked")
                && text.contains("Select a module")
                && text.contains("UI")
                && !text.contains("Option details"),
            "{text}"
        );
        assert!(
            !text.contains("args:") && !text.contains("output:"),
            "{text}"
        );
    }
}

/// 【全屏视图】【表格避让】宽表格按扣除引导列后的净宽排版，右缘不顶上概览轨道。
#[test]
fn wide_table_stays_inside_fullscreen_body_width() {
    let mut store = TranscriptStore::new(1_000);
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Content,
        text: [
            "| 项目 | 内容 |",
            "|---|---|",
            &format!("| 很长 | {} |", "一段不可拆单词ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
            "",
        ]
        .join("\n"),
    });
    store.finalize_live_tail();
    let width = 40usize;
    let document = store.render_fullscreen(width, &options(), &HashSet::new());
    let lines = plain(&document);
    let joined = lines.join("\n");
    assert!(
        joined.contains('┐') && joined.contains('┘'),
        "表格右边框必须完整可见: {joined}"
    );
    for line in &lines {
        let columns = crate::render::table::visible_width(line);
        assert!(
            columns <= width,
            "fullscreen table line exceeds body width {width}: {columns} {line:?}"
        );
    }
}

/// 【终端】【标题控制】无论展开与否，工具控件仅位于首行而非正文或末尾。
/// 参数: 无；返回: 无。
#[test]
fn control_rows_stay_on_headers() {
    let mut store = command_store();
    for expanded in [
        HashSet::new(),
        HashSet::from([ParagraphKey::activity(0, false)]),
    ] {
        let document = store.render_fullscreen(80, &options(), &expanded);
        let span = &document.paragraphs[0];
        assert_eq!(span.control, span.start);
        assert_eq!(
            document
                .paragraph_control_at(span.control)
                .map(|span| span.key),
            Some(span.key)
        );
        for row in span.start + 1..span.end {
            assert!(document.paragraph_control_at(row).is_none());
        }
    }
}

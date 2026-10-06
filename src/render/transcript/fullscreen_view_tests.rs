use super::*;
use crate::llm::ChatStreamChunk;
use crate::render::activity_animation::strip_ansi_for_test;
use crate::render::transcript::ParagraphPart;
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
    let collapsed = store.render_fullscreen(80, &options(), &HashSet::new());
    let keys = collapsed
        .paragraphs
        .iter()
        .map(|span| span.key)
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 2, "两段思考都应可折叠");
    assert!(collapsed.paragraphs.iter().all(|span| !span.expanded));
    let joined = plain(&collapsed).join("\n");
    assert!(!joined.contains("first question-think-10"), "{joined}");

    let expanded = store.render_fullscreen(80, &options(), &HashSet::from([keys[0]]));
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
        Some(LIVE_PARAGRAPH_KEY)
    );
    let open = store.render_fullscreen(80, &options(), &HashSet::from([LIVE_PARAGRAPH_KEY]));
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

/// 命令卡片拆成命令行与输出两段，两段互不重叠，分界标记不进正文。
#[test]
fn command_card_splits_into_command_and_output_paragraphs() {
    use crate::render::render_expand::{ExpandPart, PART_BOUNDARY};
    let mut store = command_store();
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let keys = document
        .paragraphs
        .iter()
        .map(|span| span.key.part)
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            ParagraphPart::Segment(ExpandPart::Command),
            ParagraphPart::Segment(ExpandPart::Output)
        ]
    );
    let (command, output) = (&document.paragraphs[0], &document.paragraphs[1]);
    assert!(command.end <= output.start, "两段不应重叠");
    let text = plain(&document);
    assert!(text[command.start].contains("$ echo step-0"), "{text:?}");
    assert!(text.iter().all(|line| !line.contains(PART_BOUNDARY)));
}

/// 点击命令行只展开命令，输出仍折叠；反之亦然。
#[test]
fn command_and_output_expand_independently() {
    use crate::render::render_expand::ExpandPart;
    let mut store = command_store();
    let collapsed = plain(&store.render_fullscreen(80, &options(), &HashSet::new())).join("\n");
    assert!(
        !collapsed.contains("step-6") && !collapsed.contains("out-15"),
        "{collapsed}"
    );
    let command_key = ParagraphKey::segment(0, ExpandPart::Command);
    let output_key = ParagraphKey::segment(0, ExpandPart::Output);
    let command_open = store.render_fullscreen(80, &options(), &HashSet::from([command_key]));
    let joined = plain(&command_open).join("\n");
    assert!(joined.contains("step-6"), "命令应展开: {joined}");
    assert!(!joined.contains("out-15"), "输出应保持折叠: {joined}");
    assert!(command_open.paragraphs[0].expanded && !command_open.paragraphs[1].expanded);
    let output_open = store.render_fullscreen(80, &options(), &HashSet::from([output_key]));
    let joined = plain(&output_open).join("\n");
    assert!(joined.contains("out-15"), "输出应展开: {joined}");
    assert!(!joined.contains("step-6"), "命令应保持折叠: {joined}");
}

/// 命令很短时没有可展开的命令段，只登记输出段，点击命令行不会白点。
#[test]
fn short_command_registers_only_the_output_paragraph() {
    use crate::render::render_expand::ExpandPart;
    let mut store = TranscriptStore::new(1_000);
    store.push_shell(
        "ls".to_string(),
        (0..30).map(|index| format!("file-{index}\n")).collect(),
        Some(0),
    );
    let document = store.render_fullscreen(80, &options(), &HashSet::new());
    let parts = document
        .paragraphs
        .iter()
        .map(|span| span.key.part)
        .collect::<Vec<_>>();
    assert_eq!(parts, vec![ParagraphPart::Segment(ExpandPart::Output)]);
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
    let key = collapsed.paragraphs.last().unwrap().key;
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
    let key = collapsed.paragraphs[0].key;
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

/// 【提问展示】【全屏回归】实际展开路径保留问题、答案与选项说明，不显示原始协议；无参数和返回值。
#[test]
fn expanded_question_uses_readable_answers() {
    for width in [40, 80] {
        let mut store = TranscriptStore::new(1000);
        store.push_tool_call("ask_question".into(), r#"{"questions":[{"header":"Scope","question":"Select a module","options":[{"label":"UI","description":"Option details"}]}]}"#.into());
        store.push_tool_result("ask_question".into(), true, r#"{"status":"answered","answers":[{"header":"Scope","question":"Select a module","answer":"UI"}]}"#.into());
        let collapsed = store.render_fullscreen(width, &options(), &HashSet::new());
        let key = collapsed.paragraphs.last().unwrap().key;
        let expanded = store.render_fullscreen(width, &options(), &HashSet::from([key]));
        let text = plain(&expanded).join("\n");
        assert!(
            text.contains("Asked")
                && text.contains("Select a module")
                && text.contains("Option details"),
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

/// 折叠段的控制行是「N lines hidden」提示；展开后末尾追加收起行作为控制行。
#[test]
fn control_rows_are_the_fold_hint_and_the_collapse_line() {
    let mut store = command_store();
    let collapsed = store.render_fullscreen(80, &options(), &HashSet::new());
    let text = plain(&collapsed);
    for span in &collapsed.paragraphs {
        assert!(text[span.control].contains("hidden"), "{:?}", text[span.control]);
        assert!(span.control >= span.start && span.control < span.end);
    }
    let output_key = collapsed.paragraphs[1].key;
    let open = store.render_fullscreen(80, &options(), &HashSet::from([output_key]));
    let text = plain(&open);
    let output = &open.paragraphs[1];
    assert_eq!(output.control, output.end - 1);
    assert!(text[output.control].contains("Show less"), "{text:?}");
    // 收起行插在输出段末尾，命令段的控制行仍是原提示
    assert!(text[open.paragraphs[0].control].contains("hidden"));
    assert_eq!(open.paragraph_control_at(output.control).map(|s| s.key), Some(output_key));
    assert!(open.paragraph_control_at(output.start).is_none() || output.start == output.control);
}

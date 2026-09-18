use super::*;

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
    let cell = crate::render::transcript::cell::HistoryCell::Reasoning(cell);
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
            // 区块前空行与箭头标题行跳过；正文必须带 gutter（`  └ ` 或四空格）
            if plain.is_empty() || plain.starts_with('›') {
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

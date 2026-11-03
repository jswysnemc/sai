use super::selection::{selected_text, Selection, TextPoint};
use crate::render::transcript::{AnsiLine, TranscriptRenderOptions, TranscriptStore};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【复制夹具】选择全部显示行，返回选中的纯文本。
/// 参数: `lines` 为已排版行，`width` 为终端列数；返回: 复制结果。
fn copy_all(lines: &[AnsiLine], width: usize) -> String {
    selected_text(
        lines,
        &Selection {
            anchor: TextPoint { row: 0, col: 0 },
            head: TextPoint {
                row: lines.len().saturating_sub(1),
                col: width,
            },
        },
        width,
    )
}

/// 【终端】【软硬换行回归】长行排版折行不进入复制结果，接口原始换行完整保留。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_joins_layout_rows_and_preserves_api_newlines() {
    let source = "abcdefghij klmnop\n第二行中文测试\n\nlast";
    let lines = AnsiLine::wrap_block(source, 8);
    assert_eq!(copy_all(&lines, 8), source);
}

/// 【终端】【正文复制回归】真实会话渲染的引导符与续行缩进不进入复制内容。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_preserves_newlines_through_fullscreen_rendering() {
    let source = "abcdefghij klmnopqrstuv\nsecond original line";
    let mut store = TranscriptStore::new(1000);
    store.push_chunk(&crate::llm::ChatStreamChunk {
        kind: crate::llm::ChatStreamKind::Content,
        text: source.into(),
    });
    store.finalize_live_tail();
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let document = store.render_fullscreen(18, &options, &Default::default());
    crate::render::tui_mode::set_override(None);
    assert_eq!(copy_all(&document.lines, 18).trim(), source);
}

/// 【终端】【代码复制回归】自动续行缩进会移除，代码原有缩进与实际换行保持不变。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_preserves_code_indentation_and_spaces_at_breaks() {
    let source = "    print('hello world')\n    return result";
    let lines = AnsiLine::wrap_block_with_continuation_indent(source, 10, 2);
    let lines = lines
        .into_iter()
        .map(|line| line.with_text(format!("  {}", line.as_str()), 2))
        .collect::<Vec<_>>();
    assert_eq!(copy_all(&lines, 12), source);
}

/// 【终端】【思考复制回归】预先折行的思考保留接口换行，引导线和内部标记不进入剪贴板。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_preserves_pre_wrapped_reasoning() {
    let source = "abcdefghijklmnopqrstuvwxyz\nsecond original line";
    let mut store = TranscriptStore::new(1000);
    store.push_chunk(&crate::llm::ChatStreamChunk {
        kind: crate::llm::ChatStreamKind::Reasoning,
        text: source.into(),
    });
    store.finalize_live_tail();
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let document = store.render_fullscreen(18, &options, &Default::default());
    crate::render::tui_mode::set_override(None);
    let region = &document.reasonings[0];
    assert_eq!(
        copy_all(&document.lines[region.start + 1..region.end], 18),
        source
    );
    assert!(document
        .lines
        .iter()
        .all(|line| !line.as_str().contains("sai-copy=")));
}

/// 【终端】【工具输出复制回归】工具预先折行和原有空段落均保留正确换行来源。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_preserves_pre_wrapped_command_output() {
    let source = "abcdefghijklmnopqrstuvwxyz\n\nsecond original line";
    let mut store = TranscriptStore::new(1000);
    store.push_tool_call(
        "run_command".into(),
        r#"{"command":"printf output"}"#.into(),
    );
    store.push_tool_result(
        "run_command".into(),
        true,
        serde_json::json!({
            "success": true, "exit_code": 0, "stdout": source, "stderr": ""
        })
        .to_string(),
    );
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Full,
    };
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let document = store.render_fullscreen(
        18,
        &options,
        &std::collections::HashSet::from([crate::render::transcript::ParagraphKey::whole(0)]),
    );
    crate::render::tui_mode::set_override(None);
    let start = document
        .lines
        .iter()
        .position(|line| line.as_str().contains("abcdefgh"))
        .unwrap();
    assert_eq!(copy_all(&document.lines[start..], 18).trim(), source);
    assert!(document
        .lines
        .iter()
        .all(|line| !line.as_str().contains("sai-copy=")));
}

/// 【终端】【列表复制回归】断点处为排版移除的词间空格在复制时恢复。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_restores_list_break_spaces() {
    let source = "• abcdefgh ij klmnopqrst";
    let lines = AnsiLine::wrap_list_item(source, 10, 2);
    assert_eq!(copy_all(&lines, 10), source);
}

/// 【终端】【反向选择回归】排版续行上的局部选区和反向拖动复制相同原文片段。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_handles_partial_and_reversed_selections() {
    let lines = AnsiLine::wrap_block("abcdefghijklmnopqrst", 6);
    let anchor = TextPoint { row: 0, col: 2 };
    let head = TextPoint { row: 2, col: 4 };
    assert_eq!(
        selected_text(&lines, &Selection { anchor, head }, 6),
        "cdefghijklmnop"
    );
    assert_eq!(
        selected_text(
            &lines,
            &Selection {
                anchor: head,
                head: anchor
            },
            6
        ),
        "cdefghijklmnop"
    );
}

/// 【终端】【隐藏范围回归】窗口省略原文中间内容时，不把两段不连续文本伪装为连续原文。
/// 参数: 无；返回: 无。
#[test]
fn copy_wrap_keeps_boundaries_when_projection_skips_source_rows() {
    let source = crate::render::transcript::FullscreenDocument {
        lines: AnsiLine::wrap_block("abcdefghijklmnopqrst", 6),
        ..Default::default()
    };
    let mut projection = super::activity_window::DocumentProjection::new(&source);
    projection.copy(0..1);
    projection.copy(2..3);
    assert_eq!(copy_all(&projection.finish().lines, 6), "abcdef\nmnopqr");
}

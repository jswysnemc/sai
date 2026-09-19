use super::{
    history_insert, layout,
    stream::{StreamState, SyncPlan},
    viewport::{InlineViewport, TerminalSize},
};
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

/// 【终端】【长表格回归】驱动真实增量协调和输出路径，禁止修改已经滚出的行
#[test]
fn long_table_scrolls_without_patching_offscreen_rows() {
    let options = TranscriptRenderOptions {
        reasoning_mode: ReasoningDisplayMode::Full,
        tool_call_mode: ToolCallDisplayMode::Summary,
    };
    let mut store = TranscriptStore::new(500);
    let mut state = StreamState::default();
    let mut viewport = InlineViewport::new();
    let size = TerminalSize { cols: 80, rows: 24 };
    let mut output = Vec::new();
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Reasoning,
        text: "REASONING-PREFIX\n".to_string(),
    });
    store.push_chunk(&ChatStreamChunk {
        kind: ChatStreamKind::Content,
        text: "INTRO-PREFIX\n\n| row | value |\n|---|---|\n".to_string(),
    });
    for index in 0..80 {
        let value = if index < 10 {
            "short"
        } else {
            "a longer value that needs additional physical lines"
        };
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: format!("| ROW-{index:03} | {value} |\n"),
        });
        let window = layout::display_window(&mut store, 80, &options, 64, state.offscreen(), 12);
        store.clear_dirty();
        let previous = viewport;
        viewport.update(size, 4, window.total.saturating_sub(state.offscreen()));
        let offscreen = state.offscreen();
        match state.sync(&window) {
            SyncPlan::Delta {
                patches,
                append,
                old_total,
                new_total,
            } => {
                assert!(
                    patches.iter().all(|(row, _)| *row >= offscreen),
                    "第 {index} 行导致已滚出内容重新排版"
                );
                assert!(new_total > old_total, "表格应随数据行增长");
                let outcome = history_insert::apply_delta(
                    &mut output,
                    &previous,
                    &viewport,
                    &patches,
                    &append,
                    old_total,
                    new_total,
                    offscreen,
                )
                .unwrap();
                viewport.apply_terminal_scroll(outcome.scrolled_rows);
                state.note_scrolled(outcome.scrolled_rows);
            }
            _ => panic!("长表格必须持续增量输出"),
        }
    }
    assert!(state.offscreen() > 80);
    store.finalize_live_tail();
    let completed = layout::display_window(&mut store, 80, &options, 64, state.offscreen(), 12);
    assert!(
        matches!(state.sync(&completed), SyncPlan::Unchanged),
        "表格完成时不能重新排列已滚出的行"
    );
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("\x1b[2J") && !output.contains("\x1b[3J"));
}

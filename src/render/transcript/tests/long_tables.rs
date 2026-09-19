use super::super::test_support::{chunk, options};
use super::super::TranscriptStore;
use crate::llm::ChatStreamKind;

/// 【终端】【长表格回归】未闭合表格出现后不能删除同一回复中已经显示的正文
#[test]
fn opening_table_keeps_preceding_response_content() {
    let mut store = TranscriptStore::new(500);
    let preface = (0..30)
        .map(|index| format!("INTRO-{index:03}\n"))
        .collect::<String>();
    store.push_chunk(&chunk(ChatStreamKind::Content, &preface));
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "\n| key | value |\n|---|---|\n| first | value |\n",
    ));
    let window = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    let rendered = window
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    assert!(
        rendered.contains("INTRO-000"),
        "流式表格不应吞掉前面的正文，当前总行数={}",
        window.total
    );
    assert!(rendered.contains("INTRO-029"));
}

/// 【终端】【长表格回归】表格持续生成时所有已完成行都应参与滚动输出
#[test]
fn growing_table_keeps_every_completed_row_before_finish() {
    let mut store = TranscriptStore::new(500);
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "| key | value |\n|---|---|\n",
    ));
    for index in 0..60 {
        store.push_chunk(&chunk(
            ChatStreamKind::Content,
            &format!("| ROW-{index:03} | value |\n"),
        ));
        let window = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
        let rendered = window
            .lines
            .iter()
            .map(|line| line.as_str())
            .collect::<String>();
        for previous in 0..=index {
            assert!(
                rendered.contains(&format!("ROW-{previous:03}")),
                "新增第 {index} 行后，第 {previous} 行被删除；总行数={}",
                window.total
            );
        }
    }
}

/// 【终端】【长表格回归】后续长单元格不能改变已滚出表格行，定稿也不能重新分配列宽
#[test]
fn long_cells_and_finalization_keep_emitted_prefix() {
    let mut store = TranscriptStore::new(500);
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "| key | description |\n|---|---|\n",
    ));
    for index in 0..10 {
        store.push_chunk(&chunk(
            ChatStreamKind::Content,
            &format!("| ROW-{index:03} | value |\n"),
        ));
        store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    }
    let before = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "| ROW-010 | A much longer description that wraps across several physical lines |\n",
    ));
    let after = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    // 1. 【终端】【长表格回归】只有最末尾的底边框可变成行间边框
    assert_eq!(
        &before.lines[..before.lines.len() - 1],
        &after.lines[..before.lines.len() - 1]
    );
    assert!(after.total > before.total);
    store.finalize_live_tail();
    let finished = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    assert_eq!(
        after.lines, finished.lines,
        "定稿不能移动已经进入滚动历史的表格行"
    );
}

/// 【终端】【长表格回归】一条回复中的多张表格独立固定布局，后续正文完整保留
#[test]
fn multiple_tables_keep_independent_layouts_and_following_text() {
    let mut store = TranscriptStore::new(500);
    for table in 0..2 {
        store.push_chunk(&chunk(
            ChatStreamKind::Content,
            if table == 0 {
                "| key | value |\n|---|---|\n"
            } else {
                "\n| key | value | notes |\n|---|---|---|\n"
            },
        ));
        for row in 0..15 {
            store.push_chunk(&chunk(
                ChatStreamKind::Content,
                &format!(
                    "| T{table}-{row:03} | value {}|\n",
                    if table == 0 { "" } else { "| notes " }
                ),
            ));
            store.display_window_with_live_cap(80, &options(), 500, 0, 12);
        }
    }
    store.push_chunk(&chunk(ChatStreamKind::Content, "\nAFTER-TABLES\n"));
    let live = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    store.finalize_live_tail();
    let done = store.display_window_with_live_cap(80, &options(), 500, 0, 12);
    assert_eq!(live.lines, done.lines);
    let text = done
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<String>();
    for table in 0..2 {
        for row in 0..15 {
            assert!(text.contains(&format!("T{table}-{row:03}")));
        }
    }
    assert!(text.contains("AFTER-TABLES"));
}

/// 【终端】【长表格回归】固定布局后出现中文和公式图片仍保持列宽，定稿保留图片
#[test]
fn frozen_table_keeps_late_math_images_and_wide_characters() {
    crate::render::terminal_image::test_override::set(Some(true), Some(false), Some(false));
    let mut store = TranscriptStore::new(500);
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "| row | formula |\n|---|---|\n",
    ));
    for row in 0..10 {
        store.push_chunk(&chunk(
            ChatStreamKind::Content,
            &format!("| {row} | value |\n"),
        ));
        store.display_window_with_live_cap(40, &options(), 500, 0, 8);
    }
    store.push_chunk(&chunk(
        ChatStreamKind::Content,
        "| 中文 | $x_{947}^2+y_{947}^2$ |\n",
    ));
    let live = store.display_window_with_live_cap(40, &options(), 500, 0, 8);
    assert!(live
        .lines
        .iter()
        .any(|line| line.as_str().contains("\x1b_Ga=p")));
    assert!(live
        .lines
        .iter()
        .all(|line| crate::render::table::visible_width(line.as_str()) <= 42));
    store.finalize_live_tail();
    let completed = store.display_window_with_live_cap(40, &options(), 500, 0, 8);
    assert_eq!(live.lines, completed.lines);
    crate::render::terminal_image::test_override::set(None, None, None);
}

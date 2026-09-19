use super::*;
use crate::render::render_width::with_render_width;

/// 【表格】【回归验证】按指定正文宽度渲染纯文本表格
/// 参数: lines 为 Markdown 行，width 为可用列数
/// 返回: 带样式的终端文本
fn render_at(lines: &[&str], width: usize) -> String {
    with_render_width(width, || {
        render_table(
            &lines
                .iter()
                .map(|line| line.to_string())
                .collect::<Vec<_>>(),
            |cell| CellContent::from_inline(cell.to_string()),
        )
    })
}

/// 【表格】【回归验证】中文与多列表格在窄窗口不能溢出或错位
#[test]
fn narrow_tables_stay_within_content_width() {
    let source = [
        "| 名称 | 状态 | 说明 |",
        "| --- | --- | --- |",
        "| 中文内容 | 正常 | 较长说明 |",
    ];
    for width in 8..=48 {
        let output = render_at(&source, width);
        let widths = output.lines().map(visible_width).collect::<Vec<_>>();
        assert!(
            widths.iter().all(|value| *value <= width),
            "width={width}, rows={widths:?}"
        );
        if output.contains('┌') {
            assert!(
                widths.iter().all(|value| *value == widths[0]),
                "width={width}, rows={widths:?}"
            );
        }
    }
}

/// 【表格】【回归验证】正文中的破折号占位行必须保留
#[test]
fn separator_like_body_rows_are_not_dropped() {
    let output = render_at(
        &["| a | b |", "| --- | --- |", "| - | - |", "| x | y |"],
        80,
    );
    assert!(output
        .lines()
        .any(|line| line.contains('│') && line.contains('-')));
}

/// 【表格】【回归验证】短内容不能被固定最小列宽撑开
#[test]
fn short_table_uses_natural_width() {
    let output = render_at(&["| a | b |", "| --- | --- |", "| 1 | 2 |"], 80);
    assert_eq!(visible_width(output.lines().next().unwrap()), 9);
}

/// 【表格】【回归验证】代码内的美元符号不能吞掉后面的单元格
#[test]
fn dollar_inside_code_does_not_open_math() {
    assert_eq!(
        split_table_cells("| `$HOME` | normal |"),
        ["`$HOME`", "normal"]
    );
}

/// 【表格】【回归验证】省略首尾竖线的表格在流式和完成时保持一致
#[test]
fn streaming_tables_accept_optional_edge_pipes_and_resize() {
    let mut table = streaming::StreamingTable::new_source_preview();
    table.push_line("名称 | 命令");
    table.push_line("--- | ---");
    table.push_line("工具 | long-command-without-spaces");
    let narrow = with_render_width(20, || table.snapshot());
    assert!(narrow.contains('┌'));
    assert!(narrow.lines().all(|line| visible_width(line) <= 20));
    let wide = with_render_width(60, || table.snapshot());
    assert!(wide.contains("long-command-without-spaces"));
    let finished = with_render_width(60, || table.finish());
    assert_eq!(wide, finished);
    assert!(!finished.contains("\x1b[1A"));
}

/// 【表格】【回归验证】无效分隔行不能把普通文本误判为表格
#[test]
fn malformed_separator_remains_source_text() {
    assert!(!is_table_separator("| --:-- | --- |"));
    assert!(!is_table_separator("| --- | |"));
    let mut table = streaming::StreamingTable::new_stable();
    table.push_line("a | b");
    let mut output = table.push_line("--- | --- | ---");
    output.push_str(&table.finish());
    assert_eq!(output, "a | b\n--- | --- | ---\n");
}

/// 【表格】【回归验证】表头中的图片协议仍然占满列宽，不受粗体前缀干扰
#[test]
fn graphics_in_header_keep_cell_padding() {
    let protocol = "\x1b_Ga=p,C=1,c=4,r=1,i=9;\x1b\\";
    let row = [
        CellContent::from_image(vec![protocol.to_string()], 4, None),
        CellContent::from_inline("x".to_string()),
    ];
    let output = render_table_row(&row, &[4, 1], true);
    assert_eq!(visible_width(output.trim_end_matches('\n')), 12);
    assert!(output.contains(&format!("{protocol}     ")));
}

/// 【表格】【回归验证】单词折行保留样式和所有原文字符
#[test]
fn wraps_words_without_splitting_short_tokens() {
    let text = "\x1b[1malpha bravo charlie\x1b[0m";
    let lines = wrap_ansi_text(text, 14);
    assert!(lines[1].contains("charlie"));
    assert!(lines.iter().all(|line| visible_width(line) <= 14));
    assert!(lines[1].starts_with("\x1b[1m"));
}

/// 【表格】【回归验证】窄屏降级继续保留图片载荷，不能退回公式源码
#[test]
fn narrow_fallback_preserves_image_cells() {
    let protocol = "\x1b_Ga=p,C=1,c=4,r=1,i=9;\x1b\\";
    let output = with_render_width(8, || {
        render_table(
            &[
                "| a | b |".to_string(),
                "| --- | --- |".to_string(),
                "| math | text |".to_string(),
            ],
            |text| {
                if text == "math" {
                    CellContent::from_image(vec![protocol.to_string()], 4, None)
                } else {
                    CellContent::from_inline(text.to_string())
                }
            },
        )
    });
    assert!(!output.contains('┌'));
    assert!(output.contains(protocol));
    assert!(output.contains("text"));
}

/// 【表格】【公式回归】真实公式转图在网格、窄屏与重排时继续生成受限尺寸的图片
#[test]
fn math_images_survive_layout_and_narrow_fallback() {
    use crate::render::{markdown_inline::render_table_cell_content, terminal_image};
    terminal_image::test_override::set(Some(true), Some(false), Some(false));
    let rows = [
        "| Formula | Meaning |".to_string(),
        "| --- | --- |".to_string(),
        "| $a_{731}^2+b_{731}^2=c_{731}^2$ | long description |".to_string(),
        "| value $x_{731}+y_{731}$ | mixed content |".to_string(),
    ];
    for width in [8, 24, 48] {
        let output = with_render_width(width, || render_table(&rows, render_table_cell_content));
        assert!(
            output.contains("\x1b_Ga=p"),
            "formula image missing at width {width}"
        );
        assert!(!output.contains("$a_{731}"));
        assert!(output.lines().all(|line| visible_width(line) <= width));
        if output.contains('┌') {
            let first = visible_width(output.lines().next().unwrap());
            assert!(output.lines().all(|line| visible_width(line) == first));
        }
    }
    terminal_image::test_override::set(None, None, None);
}

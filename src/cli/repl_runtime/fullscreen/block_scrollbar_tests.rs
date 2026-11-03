use super::*;
use crate::render::ansi_clip::strip_ansi;

/// 【终端】【占比回归】覆盖不同正文长度以及首、中、末阅读位置。
/// 参数: 无；返回: 无。
#[test]
fn thumb_tracks_visible_fraction_and_scroll_position() {
    assert_eq!(thumb_range(5, 0, 10), 0..2);
    assert_eq!(thumb_range(5, 3, 10), 1..3);
    assert_eq!(thumb_range(5, 5, 10), 3..5);
    assert_eq!(thumb_range(5, 0, 20), 0..1);
    assert_eq!(thumb_range(5, 8, 20), 2..3);
    assert_eq!(thumb_range(5, 15, 20), 4..5);
    assert_eq!(thumb_range(5, usize::MAX, 20), 4..5);
    assert_eq!(thumb_range(5, 995, 1000), 4..5);
}

/// 【终端】【短输出回归】全文可见时滑块覆盖整个轨道，空正文没有滑块。
/// 参数: 无；返回: 无。
#[test]
fn fully_visible_body_fills_the_track() {
    assert_eq!(thumb_range(0, 0, 0), 0..0);
    assert_eq!(thumb_range(0, 0, 20), 0..0);
    assert_eq!(thumb_range(1, 0, 1), 0..1);
    assert_eq!(thumb_range(5, 0, 5), 0..5);
}

/// 【终端】【正文样式回归】轨道仅修改行首竖线，嵌套缩进与语法色完整保留。
/// 参数: 无；返回: 无。
#[test]
fn gutter_color_preserves_text_and_body_styles() {
    let prefix = "  \x1b[2;36m  ";
    let body = " \x1b[1;35m代码 │ 文本\x1b[0m";
    let original = AnsiLine::new(format!("{prefix}│{body}"));
    let tinted = tint_gutter(&original, (103, 193, 207));
    assert_eq!(strip_ansi(tinted.as_str()), strip_ansi(original.as_str()));
    assert_eq!(
        tinted.as_str(),
        format!("{prefix}\x1b[22m\x1b[38;2;103;193;207m│\x1b[0m\x1b[2;36m{body}")
    );
}

/// 【终端】【引导线识别回归】正文中的竖线和不带引导线的窄终端续行不参与着色。
/// 参数: 无；返回: 无。
#[test]
fn body_pipes_and_lines_without_gutter_remain_unchanged() {
    for text in ["", "    ", "\x1b[36m正文 │ 内容\x1b[0m", "续行"] {
        let line = AnsiLine::new(text.to_string());
        assert_eq!(tint_gutter(&line, (103, 193, 207)), line);
    }
}

/// 【终端】【样式重置回归】恢复引导线之前的当前样式，不重新应用已经清除的属性。
/// 参数: 无；返回: 无。
#[test]
fn gutter_restores_only_active_styles_after_reset() {
    let original = AnsiLine::new("\x1b[1;31m\x1b[0m\x1b[2;36m  │ text".to_string());
    let tinted = tint_gutter(&original, (103, 193, 207));
    assert!(tinted.as_str().ends_with("│\x1b[0m\x1b[2;36m text"));
}

/// 【终端】【代码复制回归】新增引导线复用视觉缩进，原始代码缩进和自动续行来源保持不变。
/// 参数: 无；返回: 无。
#[test]
fn output_gutter_preserves_code_copy_and_width() {
    use super::super::selection::{selected_text, Selection, TextPoint};
    let source = "    print('hello world')\n    return result";
    let mut lines = AnsiLine::wrap_block_with_continuation_indent(source, 10, 2)
        .into_iter()
        .map(|line| line.with_text(format!("  {}", line.as_str()), 2))
        .collect::<Vec<_>>();
    let widths = lines
        .iter()
        .map(|line| crate::render::ansi_clip::ansi_visible_width(line.as_str()))
        .collect::<Vec<_>>();
    let rows = (0..lines.len()).map(|row| (row, 0)).collect::<Vec<_>>();
    decorate_output(&mut lines, &rows, 0, 100);
    assert_eq!(
        lines
            .iter()
            .map(|line| crate::render::ansi_clip::ansi_visible_width(line.as_str()))
            .collect::<Vec<_>>(),
        widths
    );
    assert_eq!(
        selected_text(
            &lines,
            &Selection {
                anchor: TextPoint { row: 0, col: 0 },
                head: TextPoint {
                    row: lines.len() - 1,
                    col: 12
                },
            },
            12
        ),
        source
    );
}

/// 【终端】【命令引导线回归】命令首行折角改为竖线，不添加第二条引导线或移动正文。
/// 参数: 无；返回: 无。
#[test]
fn command_corner_becomes_a_single_vertical_gutter() {
    let mut line = AnsiLine::new("  \x1b[2m└ output\x1b[0m".into());
    line.copy.content_start = 4;
    let decorated = output_gutter(&line, 2);
    assert_eq!(strip_ansi(decorated.as_str()), "  │ output");
    assert_eq!(decorated.copy, line.copy);
}

/// 【终端】【续行引导回归】行号续行和原始空行始终使用固定引导列，复制偏移不控制视觉位置。
/// 参数: 无；返回: 无。
#[test]
fn output_gutter_stays_aligned_across_wrapped_and_blank_rows() {
    let source = "12  very long numbered output line\n\nnext row";
    let mut lines = AnsiLine::wrap_block_with_continuation_indent(source, 12, 4)
        .into_iter()
        .map(|line| line.with_text(format!("  {}", line.as_str()), 2))
        .collect::<Vec<_>>();
    let rows = (0..lines.len()).map(|row| (row, 0)).collect::<Vec<_>>();
    decorate_output(&mut lines, &rows, 0, 100);
    for line in &lines {
        assert!(strip_ansi(line.as_str()).starts_with('│'));
    }
}

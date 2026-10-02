use super::AnsiLine;
use unicode_width::UnicodeWidthChar;

/// 【终端】【差异测试】读取每个显示列的背景；参数为 ANSI 行，返回背景色列数组。
fn background_columns(text: &str) -> Vec<usize> {
    let mut columns = Vec::new();
    let mut background = 0;
    let mut index = 0;
    while index < text.len() {
        if text[index..].starts_with('\x1b') {
            let end = crate::render::terminal_image::escape_sequence_end(text, index);
            let sequence = &text[index..end];
            if sequence == "\x1b[K" {
                break;
            }
            if crate::render::ansi_style::is_reset_sgr(sequence) {
                background = 0;
            }
            if let Some(color) = sequence
                .strip_prefix("\x1b[48;5;")
                .and_then(|s| s.strip_suffix('m'))
            {
                background = color.parse().unwrap();
            }
            index = end;
        } else {
            let ch = text[index..].chars().next().unwrap();
            columns.extend(std::iter::repeat_n(background, ch.width().unwrap_or(0)));
            index += ch.len_utf8();
        }
    }
    columns
}

/// 【终端】【差异测试】验证中英文长行对齐后每行保留相同的左右边界；无参数和返回值。
#[test]
fn wrapped_diff_background_has_stable_edges_after_alignment() {
    for width in [16, 24, 40] {
        for body in ["abcdefghij".repeat(12), "中文长行".repeat(20)] {
            let source = format!(" \x1b[48;5;22m1 +  {body}\x1b[K\x1b[0m");
            let lines = AnsiLine::wrap_block_with_right_margin_and_continuation_indent(
                &source, width, 3, 6,
            );
            assert!(lines.len() > 1);
            for line in lines {
                let aligned = crate::render::content_indent::align_to_guide_column(line.as_str());
                let backgrounds = background_columns(&aligned);
                assert_eq!(backgrounds.len(), width + 2);
                assert_eq!(&backgrounds[..3], &[0, 0, 0]);
                assert!(backgrounds[3..].iter().all(|bg| *bg == 22));
            }
        }
    }
}

/// 【终端】【差异测试】强调片段跨行时仅正文保留强调色，缩进和尾部补齐使用基础底色；无参数和返回值。
#[test]
fn intraline_emphasis_does_not_color_gutter_or_padding() {
    let source = " \x1b[48;5;22m1 +  \x1b[48;5;28m中文中文中文中文\x1b[48;5;22m\x1b[K\x1b[0m";
    let lines = AnsiLine::wrap_block_with_right_margin_and_continuation_indent(source, 11, 3, 6);
    assert!(lines.len() > 1);
    for (index, line) in lines.iter().enumerate() {
        let backgrounds = background_columns(line.as_str());
        assert_eq!(backgrounds.len(), 11);
        assert_eq!(backgrounds[0], 0);
        assert!(backgrounds[1..6].iter().all(|bg| *bg == 22));
        if index + 1 < lines.len() {
            assert_eq!(backgrounds[10], 22);
            assert_eq!(backgrounds[6], 28);
        }
    }
}

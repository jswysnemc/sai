use super::*;
use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};

/// 【终端】【前缀测试】检查实际补丁、读取、列表和代码缩进，不使用固定行号位数。
/// 参数: 无；返回: 无。
#[test]
fn detects_actual_body_columns() {
    for (line, expected) in [
        ("  82 -    format!(...)", 10),
        (" 105 +    let x = 1;", 10),
        ("  82  println!(...)", 6),
        ("105  let x = 1;", 5),
        ("120          assert_eq!(...)", 13),
        ("9            assert_eq!(...)", 13),
        ("120  -1", 5),
        ("120  \tlet x = 1;", 8),
        ("  • item", 4),
        ("  - item", 4),
        ("regular text", 0),
    ] {
        assert_eq!(continuation_column(line), expected, "{line:?}");
    }
}

/// 【终端】【对齐测试】补丁和读取续行精确对齐代码首列，并保持所有字符。
/// 参数: 无；返回: 无。
#[test]
fn wrapped_code_aligns_exactly_and_preserves_text() {
    for prefix in ["  82 -    ", " 105 +    ", "120          ", "120  "] {
        let text = format!("{prefix}{}", "x".repeat(60));
        let lines = wrap_aligned_block(&text, 25);
        for line in &lines {
            let visible = strip_ansi(line.as_str());
            assert_eq!(visible.find('x'), Some(prefix.len()), "{visible:?}");
            assert!(ansi_visible_width(line.as_str()) <= 25);
        }
        assert_eq!(
            lines
                .iter()
                .map(|line| strip_ansi(line.as_str()).matches('x').count())
                .sum::<usize>(),
            60
        );
    }
}

/// 【终端】【颜色测试】续行缩进使用补丁底色，窄窗口仍给正文保留空间。
/// 参数: 无；返回: 无。
#[test]
fn wrapped_diff_keeps_background_and_narrow_width_budget() {
    let text = format!(
        "\x1b[48;5;22m 120 +        {}\x1b[K\x1b[0m",
        "中x".repeat(30)
    );
    for width in [6, 12, 25, 80] {
        let lines = wrap_aligned_block(&text, width);
        assert!(lines.len() > 1);
        for line in lines {
            assert!(
                ansi_visible_width(line.as_str()) <= width,
                "{:?}",
                line.as_str()
            );
            assert!(line.as_str().contains("48;5;22"));
        }
    }
}

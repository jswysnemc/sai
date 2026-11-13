use super::line::AnsiLine;
use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};

/// 【终端】【行号续行回归】普通工具折行入口同样保留行号后的正文列。
#[test]
fn numbered_continuations_stay_out_of_the_number_column() {
    for prefix in ["100  ", "98\t", "120  \t"] {
        let source = format!("{prefix}{}", "x".repeat(60));
        let rows = AnsiLine::wrap_block(&source, 24);
        let first = strip_ansi(rows[0].as_str()).find('x').unwrap();
        for row in &rows {
            assert_eq!(
                strip_ansi(row.as_str()).find('x'),
                Some(first),
                "{:?}",
                row.as_str()
            );
            assert!(ansi_visible_width(row.as_str()) <= 24);
        }
        assert_eq!(
            rows.iter()
                .map(|row| strip_ansi(row.as_str()).matches('x').count())
                .sum::<usize>(),
            60
        );
    }
}

/// 【终端】【引导线续行回归】续行保持参考线，数字列留空，正文列不移动。
#[test]
fn guided_numbered_continuations_keep_the_guide_column() {
    let rows = AnsiLine::wrap_block(&format!("  │ 100  {}", "x".repeat(60)), 24);
    assert!(rows.len() > 1);
    for row in &rows {
        let plain = strip_ansi(row.as_str());
        assert_eq!(plain.find('│'), Some(2), "{plain:?}");
        assert_eq!(plain.chars().position(|ch| ch == 'x'), Some(9), "{plain:?}");
    }
}

/// 【终端】【路径续行回归】长 Windows 路径的续行从字段正文列开始。
#[test]
fn path_field_continuations_preserve_the_value_column() {
    let source = format!("File: D:\\code\\{}\\file.java", "LongDirectory".repeat(5));
    let rows = AnsiLine::wrap_block(&source, 30);
    assert!(rows.len() > 1);
    for row in rows.iter().skip(1) {
        assert!(
            strip_ansi(row.as_str()).starts_with("      "),
            "{:?}",
            row.as_str()
        );
        assert!(ansi_visible_width(row.as_str()) <= 30);
    }
}

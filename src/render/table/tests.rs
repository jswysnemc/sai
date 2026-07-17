use super::*;
use crate::render::markdown::render_table_cell_content;
use crate::render::style::{INLINE_CODE_STYLE, RESET};
use crossterm::terminal;

#[test]
fn table_uses_thin_box_borders() {
    let output = sample_table(&["| a | b |", "| - | - |", "| 1 | 2 |"]);

    assert!(output.contains('┌'));
    assert!(output.contains('┬'));
    assert!(output.contains('├'));
    assert!(output.contains('┼'));
    assert!(output.contains('└'));
    assert!(output.contains('│'));
    assert!(output.contains('─'));
}

#[test]
fn table_draws_middle_borders_between_all_rows() {
    let output = sample_table(&["| a | b |", "| - | - |", "| 1 | 2 |", "| 3 | 4 |"]);

    assert_eq!(output.matches('├').count(), 2);
}

#[test]
fn readable_table_min_width_returns_expected_values() {
    assert_eq!(readable_table_min_width(0), 0);
    assert_eq!(readable_table_min_width(1), 16);
    assert_eq!(readable_table_min_width(2), 14);
    assert_eq!(readable_table_min_width(3), 10);
    assert_eq!(readable_table_min_width(4), 10);
    assert_eq!(readable_table_min_width(5), 8);
}

#[test]
fn short_tables_use_content_width() {
    let output = sample_table(&[
        "| 项目 | 内容 |",
        "|---|---|",
        "| 名字 | Sai |",
        "| 年龄 | 18 |",
    ]);
    let terminal_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100);
    let widest = output.lines().map(visible_width).max().unwrap_or(0);

    assert!(widest < terminal_width / 2, "table too wide: {widest}");
}

#[test]
fn wraps_wide_table_cells_to_terminal_width() {
    let output = render_table(
        &[
            "| 项目 | 内容 |".to_string(),
            "|---|---|".to_string(),
            format!("| 很长 | {} |", "这是一段非常长的内容".repeat(20)),
        ],
        render_table_cell_content,
    );
    let terminal_width = terminal::size()
        .map(|(width, _)| usize::from(width))
        .unwrap_or(100);

    for line in output.lines() {
        assert!(
            visible_width(line) < terminal_width,
            "line too wide: {line}"
        );
    }
    assert!(output.lines().count() > 5);
}

#[test]
fn visible_width_counts_wide_symbols() {
    assert_eq!(visible_width("\u{2705} ok"), 5);
    assert_eq!(visible_width("\u{1f5a5}\u{fe0f} system"), 9);
}

#[test]
fn table_rows_keep_identical_width_with_wide_symbols() {
    let output = render_table(
        &[
            "| 操作 | 结果 |".to_string(),
            "|---|---|".to_string(),
            "| background_command | \u{2705} PID 832048 |".to_string(),
            "| \u{1f5a5}\u{fe0f} 系统 | \u{1f7e2} 正常 |".to_string(),
        ],
        render_table_cell_content,
    );
    let widths = output.lines().map(visible_width).collect::<Vec<_>>();
    let first = widths.first().copied().expect("expected table output");

    assert!(widths.iter().all(|width| *width == first), "{widths:?}");
}

#[test]
fn wrap_ansi_text_preserves_inline_code_style_across_lines() {
    let text = format!("{INLINE_CODE_STYLE}sudo pacman -S neovim{RESET}");

    let lines = wrap_ansi_text(&text, 12);

    assert!(lines.len() > 1);
    assert!(lines[0].contains(INLINE_CODE_STYLE));
    assert!(lines[0].ends_with(RESET));
    assert!(lines[1].starts_with(INLINE_CODE_STYLE));
    assert_eq!(
        strip_ansi_for_test(&lines.join("")),
        "sudo pacman -S neovim"
    );
}

#[test]
fn image_cell_placeholder_lines_reserve_column_width() {
    let mut output = String::new();
    push_image_cell_line(&mut output, "", 12, 10);
    assert_eq!(output, " ".repeat(10));

    let mut output = String::new();
    push_image_cell_line(&mut output, "abc", 3, 6);
    assert_eq!(output, "abc   ");
}

#[test]
fn graphics_protocol_line_emits_payload_only() {
    let mut output = String::new();
    let protocol = "\x1b_Gf=100,a=T,q=2,c=8,r=2;abc\x1b\\";

    push_image_cell_line(&mut output, protocol, 8, 10);

    assert_eq!(output, protocol);
    assert!(!output.contains("\x1b[8C"));
}

#[test]
fn pure_math_table_cells_keep_border_structure() {
    let output = sample_table(&[
        "| 名称 | 公式 | 说明 |",
        "|---|---|---|",
        "| 勾股 | $a^2+b^2=c^2$ | 直角 |",
        "| 欧拉 | $e^{i\\\\pi}+1=0$ | 恒等式 |",
    ]);
    let border_widths = output
        .lines()
        .filter(|line| line.contains('─'))
        .map(visible_width)
        .collect::<Vec<_>>();

    assert!(output.contains('┌'));
    assert!(output.contains('│'));
    if let Some(first) = border_widths.first() {
        assert!(border_widths.iter().all(|width| width == first));
    }
}

#[test]
fn split_table_cells_keeps_pipes_inside_math() {
    let cells = split_table_cells(r#"| 概率 | 全概率 | $P(B)=\sum_i P(A_i)P(B|A_i)$ | 通过 |"#);

    assert_eq!(cells.len(), 4);
    assert_eq!(cells[2], r"$P(B)=\sum_i P(A_i)P(B|A_i)$");
}

#[test]
fn split_table_cells_keeps_escaped_pipes() {
    let cells = split_table_cells(r#"| a \| b | c |"#);

    assert_eq!(cells, vec![r"a \| b", "c"]);
}

#[test]
fn math_with_pipe_table_keeps_column_count() {
    let output = sample_table(&[
        "| 名称 | 公式 | 状态 |",
        "|---|---|---|",
        r#"| 全概率 | $P(B)=\sum_i P(A_i)P(B|A_i)$ | ok |"#,
    ]);
    let data_line = output
        .lines()
        .find(|line| line.contains('│') && !line.contains('─'))
        .expect("data line");

    assert!(data_line.matches('│').count() >= 4);
    assert!(!output.contains(r"$P(B)=\sum_i"));
}

/// 渲染测试使用的简单 Markdown 表格。
///
/// 参数:
/// - `lines`: 表格原始行
///
/// 返回:
/// - 终端表格文本
fn sample_table(lines: &[&str]) -> String {
    render_table(
        &lines
            .iter()
            .map(|line| (*line).to_string())
            .collect::<Vec<_>>(),
        render_table_cell_content,
    )
}

/// 去除测试文本中的终端转义序列。
///
/// 参数:
/// - `text`: ANSI 文本
///
/// 返回:
/// - 可见文本
fn strip_ansi_for_test(text: &str) -> String {
    let mut output = String::new();
    let mut escape = false;
    let mut csi = false;
    for ch in text.chars() {
        if ch == '\x1b' {
            escape = true;
            csi = false;
        } else if escape {
            if csi {
                if ('@'..='~').contains(&ch) {
                    escape = false;
                }
            } else if ch == '[' {
                csi = true;
            } else if ch == '\\' || ch == 'm' {
                escape = false;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

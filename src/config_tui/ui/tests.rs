//! 配置界面 UI 原语单元测试：宽度度量、边框标题窄屏安全截断与状态栏压缩。

use super::*;

#[test]
fn display_width_matches_unicode_width() {
    assert_eq!(display_width("abc"), 3);
    assert_eq!(display_width("供应商"), 6);
    assert_eq!(display_width("a供b"), 4);
    assert_eq!(display_width(""), 0);
}

#[test]
fn truncate_respects_display_width() {
    assert_eq!(truncate("abcdef", 10), "abcdef");
    assert_eq!(truncate("abcdef", 4), "abc…");
    assert_eq!(truncate("供应商配置", 6), "供应…");
}

#[test]
fn pad_fills_to_display_width() {
    assert_eq!(display_width(&pad("供应商", 10)), 10);
    assert_eq!(display_width(&pad("abc", 5)), 5);
}

#[test]
fn wrap_text_breaks_on_width_and_newlines() {
    assert_eq!(wrap_text("abcdef", 3), vec!["abc", "def"]);
    assert_eq!(wrap_text("ab\ncd", 10), vec!["ab", "cd"]);
}

/// 【配置界面】【ANSI截断测试】验证带 ANSI 的截断只统计可见宽度且保留起始样式序列。
#[test]
fn truncate_ansi_counts_visible_width_only() {
    let accent = ThemePalette::current().accent;
    let styled = format!("{accent}abc{RESET}def");
    let output = truncate_ansi(&styled, 4);
    assert!(output.contains("abc"));
    assert!(output.contains('d'));
    assert!(!output.contains("ef"));
    assert!(output.starts_with(accent));
}

/// 【配置界面】【80列标题防破框测试】验证超长中文与含 ANSI 序列的标题在 80 列框内安全截断且保留右上角至少 2 列边框余量。
#[test]
fn box_title_truncates_safely_within_narrow_border_budget() {
    let long_zh = " 知识库管理 · 文件: 42  大小 1280.4 KB  目录 /home/user/.local/share/sai/knowledge/files/workspace ";
    let badge = format_box_title(long_zh, 78).expect("78 列宽框应能绘制截断标题");
    // 左侧偏移 2 列 + 徽标可见宽度 + 右上角至少 2 列边框余量 <= 78
    assert!(2 + ansi_width(&badge) + 2 <= 78);
    assert!(strip_ansi_plain(&badge).contains('…'));

    let palette = ThemePalette::current();
    let styled_long = format!(
        "{}PROVIDERS{RESET} · {}very-long-organization-and-endpoint-summary-title-for-80-col{RESET}",
        palette.brand, palette.muted
    );
    let styled_badge = format_box_title(&styled_long, 40).expect("40 列宽框应能截断含 ANSI 标题");
    assert!(2 + ansi_width(&styled_badge) + 2 <= 40);
    assert!(strip_ansi_plain(&styled_badge).contains('…'));

    assert!(format_box_title("TINY", 8).is_none());
}

/// 【配置界面】【底栏压缩测试】验证窄屏放不下时优先丢弃中间段并保留末尾返回键。
#[test]
fn status_bar_keeps_the_trailing_back_key_when_narrow() {
    let help = super::super::theme::help_line(&[
        ("\u{2191}\u{2193}", "move"),
        ("Enter", "open"),
        ("d", "delete"),
        ("q", "back"),
    ]);
    let full_width = ansi_width(&help);
    let narrowed = fit_status_bar(&help, 20);

    assert!(ansi_width(&narrowed) <= 20);
    assert!(narrowed.contains("back"), "返回键不能被截掉：{narrowed}");
    assert!(full_width > 20);
}

/// 【配置界面】【底栏保持测试】验证宽度充足时不改写底栏文本。
#[test]
fn status_bar_is_untouched_when_it_fits() {
    let help = super::super::theme::help_line(&[("q", "back")]);
    assert_eq!(fit_status_bar(&help, 80), help);
}

//! 全屏启动品牌区的信息层次、窄屏适配与字体无关绘制测试。

use super::welcome_cell::{display_lines, WelcomeCell};
use crate::config::TuiMode;
use crate::render::{ansi_clip::strip_ansi, tui_mode};

/// 【终端欢迎】【样例信息】构造模型名和中文目录，覆盖宽字符和窄屏换行。
/// 参数: 无；返回: 启动信息。
fn sample() -> WelcomeCell {
    WelcomeCell {
        version: "0.2.13".into(),
        model: "antigravity/gemini-3.8-flash".into(),
        directory: "~/workspace/中文/demo".into(),
        permissions: "YOLO".into(),
    }
}

/// 【终端欢迎】【品牌层次】宽屏只占三行，名称加粗、版本弱化、权限用风险色，标志不依赖字体。
/// 参数: 无；返回: 无，品牌或关键字段缺失时断言失败。
#[test]
fn fullscreen_welcome_has_compact_brand_and_runtime_details() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let rows = display_lines(&sample(), 80);
    tui_mode::set_override(None);
    assert_eq!(rows.len(), 3);
    assert!(strip_ansi(rows[0].as_str()).starts_with("╭──╴"));
    assert!(strip_ansi(rows[1].as_str()).starts_with("╰──╮"));
    assert!(strip_ansi(rows[2].as_str()).starts_with("╶──╯"));
    let text = rows
        .iter()
        .map(|row| row.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    for expected in [
        "sai",
        "v0.2.13",
        "antigravity/gemini-3.8-flash",
        "~/workspace/中文/demo",
        "YOLO",
    ] {
        assert!(text.contains(expected), "{text}");
    }
    assert!(text.contains("\x1b[38;5;208mYOLO"));
    assert!(text.contains("\x1b[2mv0.2.13"));
    assert!(!text.contains("\x1b[48;") && !text.contains("\x1b_G"));
}

/// 【终端欢迎】【响应宽度】窄屏完整换行模型和目录，不截断内容、不溢出或输出图片协议。
/// 参数: 无；返回: 无，字段丢失或行宽越界时断言失败。
#[test]
fn narrow_welcome_keeps_details_without_column_overflow() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    for width in [1, 2, 3, 8, 20, 32, 40, 48, 80, 120] {
        let rows = display_lines(&sample(), width);
        assert!(rows
            .iter()
            .all(|row| crate::render::ansi_clip::ansi_visible_width(row.as_str()) <= width));
        if width >= 20 {
            let plain = rows
                .iter()
                .map(|row| {
                    let plain = strip_ansi(row.as_str());
                    let text = if width >= 40 {
                        plain.chars().skip(8).collect::<String>()
                    } else {
                        plain
                    };
                    text.trim().to_string()
                })
                .collect::<String>();
            assert!(plain.contains("antigravity/gemini-3.8-flash"), "{plain}");
            assert!(plain.contains("~/workspace/中文/demo"), "{plain}");
            assert!(plain.contains("YOLO"), "{plain}");
        }
    }
    tui_mode::set_override(None);
}

/// 【终端欢迎】【正文对齐】完整会话渲染后品牌三行保持同一基线，不把底部短线当成工具。
/// 参数: 无；返回: 无，品牌笔画错位时断言失败。
#[test]
fn fullscreen_document_keeps_all_logo_rows_aligned() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let mut store = super::store::TranscriptStore::new(100);
    store.push_welcome(sample());
    let document =
        store.render_fullscreen(80, &super::test_support::options(), &Default::default());
    tui_mode::set_override(None);
    for (row, stroke) in document.lines.iter().zip(["╭──╴", "╰──╮", "╶──╯"])
    {
        assert!(strip_ansi(row.as_str()).starts_with(&format!("  {stroke}")));
    }
}

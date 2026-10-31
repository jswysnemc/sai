//! 语法缓存、增删行标记和横向滚动的回归测试。

use super::git_preview_code::{cache, render, skip_columns};
use super::git_preview_patch::parse;
use super::sidebar_palette::{bg, fg, SidebarPalette};
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width, strip_ansi};

const KEYWORD: &str = "\x1b[38;2;196;167;231m";
const NUMBER: &str = "\x1b[38;2;246;193;119m";
const STRING: &str = "\x1b[38;2;166;214;160m";
const COMMENT: &str = "\x1b[2m\x1b[38;2;110;106;134m";

/// 【审查测试】【语法着色】关键字、数字和字符串使用原有语法色，增删标记保持红绿。
/// 参数: 无；返回: 无。
#[test]
fn cached_syntax_keeps_change_markers_and_original_code() {
    let mut content = parse("@@ -1 +1 @@\n-let old = 42;\n+let new = \"text\";\n", false);
    cache(&mut content, "src/example.rs");
    assert_eq!(content.code.len(), content.lines.len());
    assert!(content.code[1].contains(KEYWORD));
    assert!(content.code[1].contains(NUMBER));
    assert!(content.code[2].contains(STRING));
    let palette = SidebarPalette::current();
    for (index, marker, color) in [(1, "-", palette.removed), (2, "+", palette.added)] {
        assert_eq!(strip_ansi(&content.code[index]), content.lines[index].text);
        let painted = render(&content.lines[index], Some(&content.code[index]), 0, 64);
        assert!(painted.contains(&fg(color)));
        assert!(strip_ansi(&painted).starts_with(&format!("   1 {marker} ")));
        let background = if marker == "+" {
            palette.added_bg
        } else {
            palette.removed_bg
        };
        assert!(painted.starts_with(&bg(background)));
        assert!(painted.ends_with("\x1b[0m"));
        assert_eq!(ansi_visible_width(&painted), 64);
        assert_eq!(
            ansi_visible_width(&clip_ansi_to_width(&painted, 12, "")),
            12
        );
    }
}

/// 【审查测试】【版本隔离】删除行的块注释不污染新增源码，后续上下文分别延续状态。
/// 参数: 无；返回: 无。
#[test]
fn removed_and_added_lines_have_separate_syntax_states() {
    let mut content = parse(
        "@@ -1,3 +1,3 @@\n-/* old comment\n+let value = 42;\n context\n-*/\n+let next = 7;\n",
        false,
    );
    cache(&mut content, "example.rs");
    assert!(content.code[1].contains(COMMENT));
    assert!(content.code[2].contains(KEYWORD));
    assert!(!content.code[2].contains(COMMENT));
    assert!(!content.code[3].contains(COMMENT));
    assert!(content.code[4].contains(COMMENT));
    assert!(content.code[5].contains(KEYWORD));
    // 1. 【审查测试】【行注释边界】真实换行结束注释，不能延续到下一条逻辑行
    let mut content = parse("@@ -0,0 +1,2 @@\n+// comment\n+let next = 7;\n", false);
    cache(&mut content, "example.rs");
    assert!(!content.code[2].contains(COMMENT));
}

/// 【审查测试】【代码偏移】横向移动后，当前词元颜色、宽字符边界和行号保持正确。
/// 参数: 无；返回: 无。
#[test]
fn horizontal_scroll_preserves_ansi_and_wide_character_alignment() {
    let source = "\x1b[31mhello\x1b[0m\x1b[32m世界\x1b[0m";
    let shifted = skip_columns(source, 6);
    assert_eq!(strip_ansi(&shifted), " 界");
    assert!(shifted.contains("\x1b[32m"));
    assert_eq!(strip_ansi(&skip_columns(source, 50)), "");
    let mut content = parse("@@ -0,0 +1 @@\n+let text = \"colored\";\n", false);
    cache(&mut content, "example.rs");
    let painted = render(&content.lines[1], Some(&content.code[1]), 15, 40);
    assert!(strip_ansi(&painted).starts_with("   1 + ored"));
    assert!(painted.contains(STRING));
}

/// 【审查测试】【整行背景】空行、横向偏移和窄屏裁剪都保持背景，上下文和片段标题不染色。
/// 参数: 无；返回: 无。
#[test]
fn change_background_fills_rows_and_resets_before_next_region() {
    let mut content = parse(
        "@@ -1,2 +1,3 @@\n-old\n+let text = \"long colored string\";\n+\n context\n",
        false,
    );
    cache(&mut content, "example.rs");
    let palette = SidebarPalette::current();
    for width in [0, 1, 8, 20, 80] {
        for horizontal in [0, 12, 200] {
            for index in [1, 2, 3] {
                let expected = if index == 1 {
                    palette.removed_bg
                } else {
                    palette.added_bg
                };
                let painted = render(
                    &content.lines[index],
                    Some(&content.code[index]),
                    horizontal,
                    width,
                );
                assert_eq!(ansi_visible_width(&painted), width);
                assert!(painted.ends_with("\x1b[0m"));
                assert!(painted.contains(&bg(expected)));
                assert!(!painted.contains('\n'));
            }
        }
    }
    for index in [0, 4] {
        assert!(
            !render(&content.lines[index], Some(&content.code[index]), 0, 80).contains("\x1b[48;")
        );
    }
}

/// 【审查测试】【背景配色】明暗终端都用蓝底表示新增、红底表示删除，不影响其他侧栏文字。
/// 参数: 无；返回: 无。
#[test]
fn change_backgrounds_are_red_and_blue_in_both_terminal_themes() {
    for background in [(0, 0, 0), (255, 255, 255)] {
        let palette = SidebarPalette::for_background(background);
        assert!(palette.added_bg.2 > palette.added_bg.0);
        assert!(palette.removed_bg.0 > palette.removed_bg.2);
        assert_ne!(palette.added_bg, palette.removed_bg);
    }
}

/// 【审查测试】【未知语言】无扩展名和未知扩展名均保留原文，不阻止差异显示。
/// 参数: 无；返回: 无。
#[test]
fn unknown_languages_fall_back_to_change_colored_text() {
    for path in ["README", "file.unknown-extension"] {
        let mut content = parse("@@ -0,0 +1 @@\n+plain text\n", false);
        cache(&mut content, path);
        assert_eq!(content.code[1], "plain text");
    }
}

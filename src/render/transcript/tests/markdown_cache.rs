use super::super::{markdown_cell, markdown_stream_cache::MarkdownStreamCache};
use crate::render::{render_width::with_render_width, table::live_layout::TableLayouts};

/// 【终端】【词级预览】未换行的正文和代码立即可见，重复预览不重复提交内容。
/// 参数：无；返回：无。
#[test]
fn partial_text_and_code_are_visible_before_newline() {
    for source in ["正在检查仓库", "first\n\nnext", "```rust\nlet value = 1;"] {
        with_render_width(80, || {
            let mut cache = MarkdownStreamCache::default();
            let mut layouts = TableLayouts::default();
            let partial = cache.render(source, 80, &mut layouts, 8);
            let plain = crate::render::ansi_clip::strip_ansi(&partial);
            let tail = source.lines().last().unwrap();
            assert!(plain.contains(tail), "{source:?}: {plain:?}");
            assert_eq!(cache.render(source, 80, &mut layouts, 8), partial);
            let complete = cache.render(&format!("{source}\n"), 80, &mut layouts, 8);
            assert_eq!(partial, complete);
        });
    }
}

/// 【终端】【增量解析】逐字符输入与完整重放一致，稳定内容只解析一次
#[test]
fn incremental_matches_replay_at_every_character() {
    let source = "# 标题\n\n**正文** $x^2$\n```rust\nlet x = 1;\n```\n\n| 项目 | 数值 |\n|---|---|\n| 中 | 12 |\n| longer words | 42 |\n\n结束\n";
    let mut cache = MarkdownStreamCache::default();
    let mut layouts = TableLayouts::default();
    let mut replay_layouts = TableLayouts::default();
    for end in source.char_indices().map(|(i, c)| i + c.len_utf8()) {
        with_render_width(40, || {
            let actual = cache.render(&source[..end], 40, &mut layouts, 8);
            let expected =
                markdown_cell::render_completed_parts(&source[..end], &mut replay_layouts, 8);
            assert_eq!(actual, expected, "source ending at {end}");
            assert_eq!(cache.render(&source[..end], 40, &mut layouts, 8), actual);
        });
    }
    assert_eq!(cache.parsed_bytes, source.len());
}

/// 【终端】【增量解析】尺寸变化和源码替换必须重建状态，避免重复或遗漏
#[test]
fn geometry_and_source_changes_invalidate_cache() {
    let mut cache = MarkdownStreamCache::default();
    let mut layouts = TableLayouts::default();
    for (source, width) in [
        ("first\n", 80),
        ("replacement\n", 80),
        ("replacement\n", 25),
    ] {
        with_render_width(width, || {
            let actual = cache.render(source, width, &mut layouts, 8);
            assert_eq!(
                actual,
                markdown_cell::render_completed_parts(source, &mut layouts, 8)
            );
        });
    }
    assert_eq!(
        cache.parsed_bytes,
        "first\n".len() + 2 * "replacement\n".len()
    );
}

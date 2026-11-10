//! 图片浮层局部覆盖回归，正文、侧栏与未遮挡图片保留原样。

use super::{image_viewer::ImageViewer, image_viewer_layout::ViewerLayout, image_viewer_view};
use super::{image_window, layout::FullscreenLayout};
use crate::render::{ansi_clip::strip_ansi, terminal_image::test_override};

/// 【图片浮窗测试】【背景保留】浮窗只覆盖自身矩形，四周文本、颜色和输入区保持原样。
/// 参数: 无；返回: 无。
#[test]
fn overlay_preserves_text_and_styles_outside_its_rectangle() {
    let layout = FullscreenLayout::compute(160, 30, 5);
    let geometry = ViewerLayout::compute(&layout);
    let mut rows = (0..layout.rows)
        .map(|row| format!("\x1b[32m{row:02}{}\x1b[0m", "界".repeat(79)))
        .collect::<Vec<_>>();
    let before = rows.clone();
    let mut viewer = ImageViewer::default();
    viewer.open("/nonexistent/sai-viewer-image.png".into());
    image_viewer_view::overlay(&mut viewer, &layout, &mut rows);
    for (row, (original, composed)) in before.iter().zip(&rows).enumerate() {
        if !(geometry.top..geometry.top + geometry.height).contains(&row) {
            assert_eq!(composed, original, "background row {row}");
        } else {
            let original = strip_ansi(original);
            let composed_text = strip_ansi(composed);
            let prefix_chars = 2 + (geometry.left - 2) / 2;
            assert_eq!(
                composed_text.chars().take(prefix_chars).collect::<String>(),
                original.chars().take(prefix_chars).collect::<String>()
            );
            assert!(
                composed_text.ends_with(&"界".repeat((160 - geometry.left - geometry.width) / 2))
            );
            assert!(composed.starts_with("\x1b[32m"));
            assert!(composed.rfind("\x1b[32m").unwrap() > geometry.width);
            assert_eq!(
                unicode_width::UnicodeWidthStr::width(composed_text.as_str()),
                160
            );
        }
    }
}

/// 【图片浮窗测试】【跨行遮挡】遮住从浮窗上方延伸下来的底图，仅保留浮窗外的图片放置。
/// 参数: 无；返回: 无。
#[test]
fn overlay_hides_intersecting_images_and_preserves_uncovered_images() {
    test_override::set(Some(true), Some(false), Some(false));
    let layout = FullscreenLayout::compute(160, 30, 5);
    let geometry = ViewerLayout::compute(&layout);
    let mut rows = vec![" ".repeat(160); 30];
    rows[geometry.top - 1] = format!(
        "{}\x1b_Ga=p,q=2,C=1,i=51,p=1,c=24,r=6\x1b\\",
        " ".repeat(geometry.left + 1)
    );
    rows[geometry.top + 2] = format!(
        "\x1b_Ga=p,q=2,C=1,i=52,p=2,c=5,r=3\x1b\\{}\x1b_Ga=p,q=2,C=1,i=53,p=3,c=5,r=3\x1b\\",
        " ".repeat(geometry.left + geometry.width + 1)
    );
    let before = rows.clone();
    let mut viewer = ImageViewer::default();
    viewer.open("/nonexistent/sai-viewer-image.png".into());
    image_viewer_view::overlay(&mut viewer, &layout, &mut rows);
    let images = image_window::placement_signature(&rows);
    assert_eq!(images.len(), 2);
    assert!(images
        .iter()
        .any(|(_, sequence)| sequence.contains("i=52,")));
    assert!(images
        .iter()
        .any(|(_, sequence)| sequence.contains("i=53,")));
    viewer.close();
    rows.clone_from(&before);
    image_viewer_view::overlay(&mut viewer, &layout, &mut rows);
    assert_eq!(rows, before);
    test_override::set(None, None, None);
}

/// 【图片浮窗测试】【内联协议】iTerm 和 Sixel 图片跨行进入浮窗时隐藏，外围图片仍显示。
/// 参数: 无；返回: 无。
#[test]
fn overlay_masks_inline_image_protocols_only_where_intersecting() {
    let layout = FullscreenLayout::compute(160, 30, 5);
    let geometry = ViewerLayout::compute(&layout);
    for payload in [
        "\x1b]1337;File=inline=1;width=6;height=3;preserveAspectRatio=1:aA==\x07",
        "\x1bPq\"1;1;48;60#0;2;100;0;0~\x1b\\",
    ] {
        let mut rows = vec![" ".repeat(160); 30];
        rows[geometry.top - 1] = format!("{}{payload}", " ".repeat(geometry.left + 1));
        rows[geometry.top + 2] = format!("{payload}{}", " ".repeat(160));
        let mut viewer = ImageViewer::default();
        viewer.open("/nonexistent/sai-viewer-image.png".into());
        image_viewer_view::overlay(&mut viewer, &layout, &mut rows);
        assert!(!rows[geometry.top - 1].contains(payload));
        assert!(rows[geometry.top + 2].contains(payload));
    }
}

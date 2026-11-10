use super::*;
use crate::agent::AgentMode;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

struct ProtocolGuard;
impl ProtocolGuard {
    /// 【输入图片】【测试协议】线程内启用指定协议，不污染其他测试。
    /// 参数: kitty 为是否启用图片；返回: 自动恢复守卫。
    fn new(kitty: bool) -> Self {
        terminal_image::test_override::set(Some(kitty), Some(false), Some(false));
        Self
    }
}
impl Drop for ProtocolGuard {
    /// 【输入图片】【测试清理】恢复线程协议覆盖；无参数和返回值。
    fn drop(&mut self) {
        terminal_image::test_override::set(None, None, None);
    }
}

/// 【输入图片】【测试草稿】创建包含图片标签的最小输入框。
/// 参数: 无；返回: 输入框。
fn frame() -> ComposerFrame {
    ComposerFrame::new(
        ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "model".into(),
            thinking: "auto".into(),
            directory: "/workspace".into(),
            branch: None,
            cache_hit_ratio: None,
            status_plugin: None,
        },
        "[image 1 20x10]".into(),
        15,
        false,
        Vec::new(),
        0,
    )
}

/// 【输入图片】【测试栅格】生成真实 PNG，避免无效载荷掩盖图形绘制错误。
/// 参数: 无；返回: 临时 PNG 文件。
fn image_file() -> tempfile::NamedTempFile {
    let mut file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(20, 10, Rgba([220, 80, 20, 255])))
        .write_to(&mut file, ImageFormat::Png)
        .unwrap();
    file
}

/// 【输入图片】【测试绘制】在指定区域绘制，返回文本输出与签名。
/// 参数: frame 为输入框，width/height/top 为区域尺寸和位置，previous 为旧签名；返回: 输出和签名。
fn draw(
    frame: &ComposerFrame,
    width: u16,
    height: u16,
    top: u16,
    previous: Option<&ComposerSignature>,
) -> (String, ComposerSignature) {
    let viewport = InlineViewport::fixed(
        TerminalSize {
            cols: width,
            rows: top + height,
        },
        top,
        height,
    );
    let mut output = Vec::new();
    let (_, signature) = frame.draw_lines(&mut output, &viewport, previous).unwrap();
    (String::from_utf8(output).unwrap(), signature)
}

/// 【输入图片】【布局回归】宽度边界、隐藏计数与可见数量上限保持一致。
/// 参数: 无；返回: 无。
#[test]
fn image_strip_width_reserves_overflow_badge() {
    assert_eq!(visible_tiles(6, 1), (1, 0));
    assert_eq!(visible_tiles(13, 2), (2, 0));
    assert_eq!(visible_tiles(13, 3), (1, 2));
    assert_eq!(visible_tiles(1, 3), (0, 3));
    assert_eq!(visible_tiles(10_000, 1000), (64, 936));
    for width in 0..80 {
        for count in 0..30 {
            let (visible, hidden) = visible_tiles(width, count);
            assert_eq!(visible + hidden, count);
            if visible > 0 {
                let used = if hidden > 0 {
                    visible * 7 + hidden.to_string().len() + 1
                } else {
                    visible * 7 - 1
                };
                assert!(used <= width);
            }
        }
    }
}

/// 【输入图片】【绘制回归】先画背景再放图片，保持输入命中映射与稳定帧零输出。
/// 参数: 无；返回: 无。
#[test]
fn image_strip_draws_after_background_and_keeps_cursor_geometry() {
    let _guard = ProtocolGuard::new(true);
    let file = image_file();
    let mut frame = frame();
    let base_height = frame.height(80);
    frame.set_images(vec![file.path().to_string_lossy().into_owned()], None);
    assert_eq!(frame.height(80), base_height + 4);
    let height = frame.height(80);
    let (output, signature) = draw(&frame, 80, height, 5, None);
    assert_eq!(signature.images.len(), 1);
    assert_eq!(signature.images[0].column, 3);
    assert_eq!(signature.images[0].row, 6);
    assert!(output.find("\x1b_Ga=p").unwrap() > output.rfind("\x1b[48;5;235m").unwrap());
    let geometry = signature.input_geometry.as_ref().unwrap();
    assert_eq!(geometry.cursor_hit(3, 10), Some(0));
    assert_eq!(geometry.cursor_hit(3, 6), None);
    let (unchanged, _) = draw(&frame, 80, height, 5, Some(&signature));
    assert!(unchanged.is_empty());
    frame.input.push('x');
    frame.cursor += 1;
    let (typing, _) = draw(&frame, 80, height, 5, Some(&signature));
    assert!(
        !typing.contains("\x1b_G"),
        "typing must not retransmit thumbnails"
    );
}

/// 【输入图片】【清理回归】移位、删除和整屏删除后补绘不影响正文图片。
/// 参数: 无；返回: 无。
#[test]
fn image_strip_cleans_only_its_placements_and_repaints_after_invalidation() {
    let _guard = ProtocolGuard::new(true);
    let file = image_file();
    let mut frame = frame();
    frame.set_images(vec![file.path().to_string_lossy().into_owned()], None);
    let height = frame.height(80);
    let (_, signature) = draw(&frame, 80, height, 0, None);
    let (moved, mut signature) = draw(&frame, 80, height, 2, Some(&signature));
    assert!(moved.contains("a=d"));
    assert!(!moved.contains("d=a"));
    signature.invalidate_images();
    let (repaint, signature) = draw(&frame, 80, height, 2, Some(&signature));
    assert!(repaint.contains("\x1b_Ga=p"));
    frame.set_images(Vec::new(), None);
    let (deleted, signature) = draw(&frame, 80, height, 2, Some(&signature));
    assert!(deleted.contains("a=d"));
    assert!(!deleted.contains("\x1b_Ga=p"));
    assert!(signature.images.is_empty());
}

/// 【输入图片】【降级回归】小窗口、不支持图形与解码失败仍保留可编辑正文。
/// 参数: 无；返回: 无。
#[test]
fn image_strip_falls_back_without_losing_draft() {
    let guard = ProtocolGuard::new(true);
    let file = image_file();
    let mut frame = frame();
    frame.set_images(vec![file.path().to_string_lossy().into_owned(); 3], None);
    let (_, short) = draw(&frame, 80, 5, 0, None);
    assert!(short.images.is_empty());
    assert!(short.input_geometry.is_some());
    let (_, narrow) = draw(&frame, 16, frame.height(16), 0, None);
    assert_eq!(narrow.images.len(), 1);
    assert!(narrow.lines.iter().any(|line| line.contains("+2")));
    frame.set_images(vec!["/nonexistent/composer-test.png".into()], None);
    let (failed, signature) = draw(&frame, 80, frame.height(80), 0, None);
    assert!(signature.images.is_empty());
    assert!(failed.contains("--"));
    assert!(failed.contains("[image 1 20x10]"));
    drop(guard);
    let _guard = ProtocolGuard::new(false);
    assert_eq!(frame.images.layout(80).height(), 0);
}

/// 【输入图片】【缓存回归】重复图片使用独立放置，宽度变化仅保留可见缓存。
/// 参数: 无；返回: 无。
#[test]
fn image_strip_caches_duplicate_sources_independently() {
    let _guard = ProtocolGuard::new(true);
    let file = image_file();
    let mut frame = frame();
    frame.set_images(vec![file.path().to_string_lossy().into_owned(); 3], None);
    let first = frame.images.layout(80);
    let second = frame.images.layout(80);
    assert_eq!(first.placements(0), second.placements(0));
    assert_ne!(first.tiles[0], first.tiles[1]);
    assert_eq!(frame.images.cache.borrow().1.len(), 3);
    let narrow = frame.images.layout(13);
    assert_eq!(narrow.tiles.len(), 1);
    assert_eq!(frame.images.cache.borrow().1.len(), 1);
}

/// 【输入图片】【命中回归】可见边缘返回对应来源，间隔、隐藏图片和失效签名不能点击。
/// 参数: 无；返回: 无。
#[test]
fn image_hits_follow_visible_tiles_and_clipping() {
    let _guard = ProtocolGuard::new(true);
    let first = image_file();
    let second = image_file();
    let sources = vec![
        first.path().to_string_lossy().into_owned(),
        second.path().to_string_lossy().into_owned(),
    ];
    let mut frame = frame();
    frame.set_images(sources.clone(), None);
    let (_, mut signature) = draw(&frame, 80, frame.height(80), 5, None);
    for (index, source) in sources.iter().enumerate() {
        for column in 3 + index * 7..9 + index * 7 {
            for row in 6..9 {
                assert_eq!(
                    signature
                        .image_source_at(column as u16, row)
                        .map(|source| &**source),
                    Some(source.as_str())
                );
            }
        }
    }
    for (column, row) in [(2, 6), (9, 6), (16, 6), (3, 5), (3, 9)] {
        assert!(signature.image_source_at(column, row).is_none());
    }
    let (_, narrow) = draw(&frame, 15, frame.height(15), 5, None);
    assert!(narrow.image_source_at(3, 6).is_some());
    assert!(narrow.image_source_at(10, 6).is_none());
    let (_, short) = draw(&frame, 80, 5, 5, None);
    assert!(short.image_source_at(3, 6).is_none());
    signature.invalidate_images();
    assert!(signature.image_source_at(3, 6).is_none());
}

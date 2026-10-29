use super::*;
use crate::render::terminal_image::{reserved_image_rows, test_override};

/// 【终端图片】【预览测试】生成非透明测试图片。
/// 参数: `width`、`height` 为像素尺寸；返回: 临时目录与图片路径。
fn fixture(width: u32, height: u32) -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preview.png");
    image::RgbaImage::from_pixel(width, height, image::Rgba([23, 117, 211, 255]))
        .save(&path)
        .unwrap();
    (directory, path.to_string_lossy().into_owned())
}

/// 【终端图片】【左侧预览测试】宽窗口不在图片前添加右对齐空白，预览仍保留全部占位行。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_image_preview_starts_at_the_left_edge() {
    test_override::set(Some(true), Some(false), Some(false));
    let (_directory, path) = fixture(640, 480);
    let output = render(&path, 80);
    assert!(!output.starts_with(' '), "right-aligned prefix remains");
    assert_eq!(
        output.split('\n').count(),
        reserved_image_rows(&output).unwrap()
    );
    test_override::set(None, None, None);
}

/// 【终端图片】【预览测试】Kitty 预览保持实际比例、左对齐和完整占位，并稳定复用放置标识。
/// 参数: 无；返回: 无。
#[test]
fn kitty_preview_is_bounded_left_aligned_and_cached() {
    test_override::set(Some(true), Some(false), Some(false));
    let (_directory, path) = fixture(640, 480);
    let output = render(&path, 80);
    let rows = reserved_image_rows(&output).unwrap();
    assert!(rows <= MAX_ROWS);
    assert_eq!(output.split('\n').count(), rows);
    assert!(!output.starts_with(' '));
    assert!(output.contains("a=t"));
    assert!(output.contains("a=p"));
    assert_eq!(output, render(&path, 80));
    let narrow = render(&path, 12);
    assert!(narrow.contains(",c=12,"));
    test_override::set(None, None, None);
}

/// 【终端图片】【数据来源测试】用户数据 URL 与文件使用相同有界布局。
/// 参数: 无；返回: 无。
#[test]
fn data_url_preview_uses_existing_kitty_renderer() {
    test_override::set(Some(true), Some(false), Some(false));
    let (_directory, path) = fixture(160, 160);
    let source = format!(
        "data:image/png;base64,{}",
        STANDARD.encode(std::fs::read(path).unwrap())
    );
    let output = render(&source, 70);
    assert!(output.contains("a=p"), "{output}");
    assert_eq!(
        output.split('\n').count(),
        reserved_image_rows(&output).unwrap()
    );
    assert!(!output.contains("data:image"));
    test_override::set(None, None, None);
}

/// 【终端图片】【布局测试】纵向图片靠左展示，保留全部协议占位行。
/// 参数: 无；返回: 无。
#[test]
fn portrait_alignment_preserves_all_reserved_rows() {
    let protocol = "\x1b_Ga=p,q=2,C=1,i=42,p=8,c=6,r=4\x1b\\\n\n\n\n";
    let output = align_left(protocol);
    assert!(output.starts_with("\x1b_G"));
    assert_eq!(output.split('\n').count(), 4);
}

/// 【终端图片】【降级布局测试】iTerm 预览按协议实际列数对齐，不把不可见载荷当作单列文本。
/// 参数: 无；返回: 无。
#[test]
fn iterm_alignment_uses_declared_width() {
    let protocol = "\x1b]1337;File=inline=1;width=12;height=4;preserveAspectRatio=1:DATA\x07\n";
    assert!(align_left(protocol).starts_with("\x1b]1337;"));
    assert_eq!(align_left(protocol).split('\n').count(), 4);
}

/// 【终端图片】【降级测试】无效来源提供可读英文提示，不泄漏数据 URL。
/// 参数: 无；返回: 无。
#[test]
fn invalid_image_source_has_semantic_fallback() {
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        assert!(render("data:image/png;base64,invalid", 80).contains("Image preview unavailable."));
    });
}

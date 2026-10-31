use super::*;
use crate::render::transcript::AnsiLine;

/// 【图片元数据测试】【排版隔离】图片来源不输出，全部占位行保留来源和实际缩进。
/// 参数: 无；返回: 无。
#[test]
fn metadata_is_removed_and_carried_across_reserved_rows() {
    let source = "data:image/png;base64,fixture";
    let image = "\x1b_Ga=p,q=2,C=1,i=42,p=8,c=6,r=4\x1b\\\n\n\n";
    let input = format!("  {}\nordinary", annotate(source, image));
    let lines = AnsiLine::wrap_block(&input, 30);
    for line in &lines[..4] {
        let image = line.image.as_ref().unwrap();
        assert_eq!(&*image.source, source);
        assert_eq!((image.column, image.width), (2, 6));
        assert!(!line.as_str().contains(PREFIX));
    }
    assert!(lines[4].image.is_none());
    let moved = lines[1].with_text("    ".into(), 2);
    assert_eq!(moved.image.unwrap().column, 4);
}

/// 【图片元数据测试】【Sixel 布局】协议尺寸必须进入占位识别，不把图片当作一个不可见字符。
/// 参数: 无；返回: 无。
#[test]
fn sixel_raster_dimensions_are_cell_bounded() {
    let protocol = "\x1bPq\"1;1;24;33#0;2;100;0;0~\x1b\\";
    assert_eq!(dimensions(protocol), (3, 3));
    assert_eq!(
        crate::render::terminal_image::reserved_image_rows(protocol),
        Some(3)
    );
}

/// 【图片元数据测试】【异常标记】无效或截断元数据不进入终端输出。
/// 参数: 无；返回: 无。
#[test]
fn malformed_metadata_is_not_emitted() {
    for input in [
        format!("before{PREFIX}invalid\x1b\\after"),
        format!("before{PREFIX}unfinished"),
    ] {
        let (text, image) = extract(&input);
        assert!(image.is_none());
        assert!(!text.contains(PREFIX));
    }
}

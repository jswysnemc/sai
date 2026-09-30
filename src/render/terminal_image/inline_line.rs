/// 行内图片最多占用的终端列数，避免超长公式把整行挤出屏幕。
const INLINE_LINE_MAX_COLS: usize = 60;

/// 【终端图片】【行内放置】把图片放进当前文本行：高度固定一行，宽度按比例计算，
/// 放置后补同宽空格推进光标，后续文字接在图片右侧而不是换到下一行。
///
/// 块级图片的渲染会在末尾追加与图片行数相同的换行来占位；行内公式复用那条
/// 路径时，公式后的文字被推到图片下方，并留下多余空行。
///
/// 参数:
/// - `path`: PNG 图片路径
///
/// 返回:
/// - 支持 Kitty 图形协议时返回放置序列与占位空格；其余终端返回 `None`，
///   由调用方改为显示公式源码
pub(crate) fn render_inline_line_image(path: &Path) -> Result<Option<String>> {
    if !supports_kitty_graphics() {
        return Ok(None);
    }
    let (cell_pw, cell_ph) = terminal_cell_pixel_size();
    let (cell_pw, cell_ph) = normalize_mono_cell_pixels(cell_pw, cell_ph);
    // 1. 去掉四周透明留白，按一行高度等比缩放
    let image = crop_transparent_bounds(&load_image_rgba(path)?);
    let cols = inline_line_cols(image.width, image.height, cell_pw, cell_ph);
    let image = pad_raster_to_cell_grid(image, cols, 1, cell_pw, cell_ph);
    let temp = tempfile::Builder::new()
        .prefix("sai-kitty-inline-")
        .suffix(".png")
        .tempfile()
        .context("failed to create temporary inline image")?;
    write_raster_png(temp.path(), &image)?;
    // 2. C=1 放置不移动光标，补空格让文本层占住同样宽度
    let mut output = encode_kitty_png(temp.path(), Some(cols), Some(1))?;
    output.push_str(&" ".repeat(cols));
    Ok(Some(output))
}

/// 计算一行高度下图片需要的列数。
///
/// 参数:
/// - `width`: 图片像素宽
/// - `height`: 图片像素高
/// - `cell_pw`: 单格像素宽
/// - `cell_ph`: 单格像素高
///
/// 返回:
/// - 列数，至少 1 列、至多 `INLINE_LINE_MAX_COLS`
fn inline_line_cols(width: usize, height: usize, cell_pw: usize, cell_ph: usize) -> usize {
    let numerator = width.max(1).saturating_mul(cell_ph.max(1));
    let denominator = height.max(1).saturating_mul(cell_pw.max(1));
    numerator
        .div_ceil(denominator)
        .clamp(1, INLINE_LINE_MAX_COLS)
}

#[cfg(test)]
mod inline_line_tests {
    use super::*;

    /// 验证列数按一行高度的宽高比计算并受上限约束。
    #[test]
    fn inline_cols_follow_aspect_ratio_for_one_row() {
        // 1 行 = 20px 高，宽 100px、高 20px 的图正好 100/10 = 10 列
        assert_eq!(inline_line_cols(100, 20, 10, 20), 10);
        // 高 40px 的图缩到一行后宽度减半
        assert_eq!(inline_line_cols(100, 40, 10, 20), 5);
        assert_eq!(inline_line_cols(1, 400, 10, 20), 1);
        assert_eq!(inline_line_cols(10_000, 10, 10, 20), INLINE_LINE_MAX_COLS);
    }

    /// 验证 Kitty 行内放置只占一行且没有换行，末尾空格与列数一致。
    #[test]
    fn kitty_inline_placement_stays_on_the_current_line() {
        test_override::set(Some(true), Some(false), Some(false));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formula.png");
        write_raster_png(
            &path,
            &RasterImage {
                pixels: vec![
                    Rgba {
                        r: 255,
                        g: 255,
                        b: 255,
                        a: 255
                    };
                    64 * 16
                ],
                width: 64,
                height: 16,
            },
        )
        .unwrap();
        let output = render_inline_line_image(&path).unwrap().unwrap();
        test_override::set(None, None, None);
        assert!(!output.contains('\n'));
        assert!(output.contains(",r=1"));
        let cols = output
            .split(",c=")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap();
        assert!(output.ends_with(&" ".repeat(cols)));
    }

    /// 验证不支持 Kitty 的终端交给调用方降级。
    #[test]
    fn non_kitty_terminals_fall_back() {
        test_override::set(Some(false), Some(true), Some(false));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.png");
        let result = render_inline_line_image(&path).unwrap();
        test_override::set(None, None, None);
        assert!(result.is_none());
    }
}

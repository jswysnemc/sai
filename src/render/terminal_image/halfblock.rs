/// 从缩放后的坐标采样原图像素。
///
/// 参数:
/// - `image`: 原图
/// - `x`: 目标 X
/// - `y`: 目标 Y
/// - `target_width`: 目标宽度
/// - `target_height`: 目标高度
///
/// 返回:
/// - 原图采样像素
fn sample_resized_pixel(
    image: &RasterImage,
    x: usize,
    y: usize,
    target_width: usize,
    target_height: usize,
) -> Rgba {
    if target_width >= image.width && target_height >= image.height {
        let source_x = (x * image.width / target_width).min(image.width.saturating_sub(1));
        let source_y = (y * image.height / target_height).min(image.height.saturating_sub(1));
        return image.pixels[source_y * image.width + source_x];
    }

    let start_x = x * image.width / target_width;
    let end_x = ((x + 1) * image.width).div_ceil(target_width);
    let start_y = y * image.height / target_height;
    let end_y = ((y + 1) * image.height).div_ceil(target_height);
    average_pixels(
        image,
        start_x.min(image.width.saturating_sub(1)),
        end_x.clamp(start_x + 1, image.width),
        start_y.min(image.height.saturating_sub(1)),
        end_y.clamp(start_y + 1, image.height),
    )
}

/// 对原图矩形区域做简单平均采样。
///
/// 参数:
/// - `image`: 原图
/// - `start_x`: 起始 X
/// - `end_x`: 结束 X
/// - `start_y`: 起始 Y
/// - `end_y`: 结束 Y
///
/// 返回:
/// - 平均像素
fn average_pixels(
    image: &RasterImage,
    start_x: usize,
    end_x: usize,
    start_y: usize,
    end_y: usize,
) -> Rgba {
    let mut total_r = 0u32;
    let mut total_g = 0u32;
    let mut total_b = 0u32;
    let mut total_a = 0u32;
    let mut count = 0u32;
    for y in start_y..end_y {
        for x in start_x..end_x {
            let pixel = image.pixels[y * image.width + x];
            total_r += u32::from(pixel.r);
            total_g += u32::from(pixel.g);
            total_b += u32::from(pixel.b);
            total_a += u32::from(pixel.a);
            count += 1;
        }
    }
    if count == 0 {
        return Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        };
    }
    Rgba {
        r: (total_r / count) as u8,
        g: (total_g / count) as u8,
        b: (total_b / count) as u8,
        a: (total_a / count) as u8,
    }
}

/// 渲染一个上/下半像素字符格。
///
/// 参数:
/// - `top`: 上半像素
/// - `bottom`: 下半像素
///
/// 返回:
/// - ANSI 文本片段
fn render_halfblock_cell(top: Rgba, bottom: Rgba) -> String {
    let top_visible = top.a >= ANSI_ALPHA_THRESHOLD;
    let bottom_visible = bottom.a >= ANSI_ALPHA_THRESHOLD;
    match (top_visible, bottom_visible) {
        (true, true) => {
            let top = blend_over_background(top);
            let bottom = blend_over_background(bottom);
            format!(
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                top.r, top.g, top.b, bottom.r, bottom.g, bottom.b
            )
        }
        (true, false) => {
            let top = blend_over_background(top);
            format!("\x1b[49m\x1b[38;2;{};{};{}m▀", top.r, top.g, top.b)
        }
        (false, true) => {
            let bottom = blend_over_background(bottom);
            format!("\x1b[49m\x1b[38;2;{};{};{}m▄", bottom.r, bottom.g, bottom.b)
        }
        (false, false) => "\x1b[0m ".to_string(),
    }
}

/// 将半透明像素合成到深色背景，避免透明 PNG 在 ANSI fallback 下发灰。
///
/// 参数:
/// - `pixel`: 原像素
///
/// 返回:
/// - 合成后的不透明像素
fn blend_over_background(pixel: Rgba) -> Rgba {
    if pixel.a == 255 {
        return pixel;
    }
    let alpha = u16::from(pixel.a);
    let inverse = 255 - alpha;
    Rgba {
        r: blend_channel(pixel.r, ANSI_FALLBACK_BG.r, alpha, inverse),
        g: blend_channel(pixel.g, ANSI_FALLBACK_BG.g, alpha, inverse),
        b: blend_channel(pixel.b, ANSI_FALLBACK_BG.b, alpha, inverse),
        a: 255,
    }
}

/// 混合单个色彩通道。
///
/// 参数:
/// - `foreground`: 前景通道
/// - `background`: 背景通道
/// - `alpha`: 前景 alpha
/// - `inverse`: 背景 alpha
///
/// 返回:
/// - 混合后的通道
fn blend_channel(foreground: u8, background: u8, alpha: u16, inverse: u16) -> u8 {
    ((u16::from(foreground) * alpha + u16::from(background) * inverse + 127) / 255) as u8
}

/// 计算 chafa 图片显示尺寸。
///
/// 返回:
/// - chafa `--size` 参数
fn chafa_size(size: &TerminalImageSize) -> Option<String> {
    if size.width_cells.is_some() || size.height_cells.is_some() {
        let width = size
            .width_cells
            .map(|value| value.min(300).to_string())
            .unwrap_or_default();
        let height = size
            .height_cells
            .map(|value| value.min(200).to_string())
            .unwrap_or_default();
        return Some(format!("{width}x{height}"));
    }
    let (cols, rows) = terminal::size().ok()?;
    let width = cols.clamp(20, 120);
    let height = (rows / 2).clamp(8, 40);
    Some(format!("{width}x{height}"))
}

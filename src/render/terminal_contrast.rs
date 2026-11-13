//! 按终端实际背景修正文字对比度，保持颜色层级和色相。

pub(crate) type Rgb = (u8, u8, u8);
pub(crate) const MIN_TEXT_CONTRAST: f64 = 4.5;

/// 【终端】【相对亮度】按 sRGB 计算感知亮度。
/// 参数：RGB 颜色；返回：0 到 1 的相对亮度。
fn luminance((red, green, blue): Rgb) -> f64 {
    let channel = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    channel(red) * 0.2126 + channel(green) * 0.7152 + channel(blue) * 0.0722
}

/// 【终端】【文字对比度】计算前景与背景的可读性比值。
/// 参数：前景、背景；返回：1 到 21 的对比度。
pub(crate) fn contrast(foreground: Rgb, background: Rgb) -> f64 {
    let first = luminance(foreground);
    let second = luminance(background);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

/// 【终端】【颜色混合】向指定端点最少量混合，保留原始色相。
/// 参数：原色、端点和混合比例；返回：混合 RGB。
pub(crate) fn blend(color: Rgb, target: Rgb, amount: f64) -> Rgb {
    let channel = |source: u8, target: u8| {
        (f64::from(source) * (1.0 - amount) + f64::from(target) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (
        channel(color.0, target.0),
        channel(color.1, target.1),
        channel(color.2, target.2),
    )
}

/// 【终端】【可读前景】保留满足要求的颜色，不足时沿最佳黑白端点最少量调整。
/// 参数：期望前景、实际背景和最小对比度；返回：修正后的前景。
pub(crate) fn readable_color_on(color: Rgb, background: Rgb, minimum: f64) -> Rgb {
    if contrast(color, background) >= minimum {
        return color;
    }
    let black = (0, 0, 0);
    let white = (255, 255, 255);
    let target = if contrast(black, background) >= contrast(white, background) {
        black
    } else {
        white
    };
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..16 {
        let midpoint = (low + high) / 2.0;
        if contrast(blend(color, target, midpoint), background) >= minimum {
            high = midpoint;
        } else {
            low = midpoint;
        }
    }
    blend(color, target, high)
}

/// 【终端】【主题强调】向对比度更高的端点强调，浅色加深、深色提亮。
/// 参数：原色、背景及强调比例；返回：至少达到正文对比度的前景。
pub(crate) fn emphasize(color: Rgb, background: Rgb, amount: f64) -> Rgb {
    let black = (0, 0, 0);
    let white = (255, 255, 255);
    let target = if contrast(black, background) >= contrast(white, background) {
        black
    } else {
        white
    };
    readable_color_on(blend(color, target, amount), background, MIN_TEXT_CONTRAST)
}

#[cfg(test)]
#[path = "terminal_contrast_tests.rs"]
mod tests;

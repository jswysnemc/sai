use super::{blend_color, RESET};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// 状态文字常态保持接近白色，窄暗带也不低于可读灰阶
const TEXT_COLOR: (u8, u8, u8) = (239, 239, 239);
const SHADE_COLOR: (u8, u8, u8) = (168, 168, 168);
/// 暗带最多覆盖可见文字列数的五分之一
const DARK_WIDTH_DIVISOR: usize = 5;
/// 各种长度的状态文字均约两秒完成一次扫描
const SHIMMER_CYCLE_FRAMES: usize = 63;

/// 【终端】【状态动效】在明亮文字上渲染从左向右移动的窄暗带。
/// 参数：`text` 为状态文字，`frame` 为动画帧号；返回：保持原文的 ANSI 流光文本。
pub(crate) fn render_activity_text(text: &str, frame: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    // 1. 【终端】【状态动效】按完整字形计算列宽，空格不增加允许压暗的文字比例
    let glyphs = text
        .graphemes(true)
        .map(|glyph| (glyph, UnicodeWidthStr::width(glyph)))
        .collect::<Vec<_>>();
    let total_width: usize = glyphs.iter().map(|(_, width)| width).sum();
    let visible_width: usize = glyphs
        .iter()
        .filter(|(glyph, _)| !glyph.chars().all(char::is_whitespace))
        .map(|(_, width)| width)
        .sum();
    let widest_glyph = glyphs.iter().map(|(_, width)| *width).max().unwrap_or(1);
    // 2. 【终端】【状态动效】为宽字形预留边缘列，整个字形变暗后仍不超过两成
    let band_width =
        (visible_width / DARK_WIDTH_DIVISOR).saturating_sub(widest_glyph.saturating_sub(1)) as f32;
    let half_width = band_width / 2.0;
    let phase = (frame % SHIMMER_CYCLE_FRAMES) as f32 / SHIMMER_CYCLE_FRAMES as f32;
    let center = phase * (total_width as f32 + band_width) - half_width;
    let mut column = 0;
    let mut output = String::from("\x1b[22m\x1b[1m");
    for (glyph, width) in glyphs {
        // 3. 【终端】【状态动效】仅对暗带内的字形调整亮度，其余文字始终保持明亮
        let position = column as f32 + width as f32 / 2.0;
        let shade = shade_strength((position - center).abs(), half_width);
        let (red, green, blue) = blend_color(TEXT_COLOR, SHADE_COLOR, shade);
        output.push_str(&format!("\x1b[38;2;{red};{green};{blue}m{glyph}"));
        column += width;
    }
    output.push_str(RESET);
    output
}

/// 【终端】【状态动效】计算余弦暗带强度，过短文字不压暗。
/// 参数：`distance` 为字形中心到暗带中心的距离，`half_width` 为暗带半宽；返回：0 到 1 的压暗比例。
fn shade_strength(distance: f32, half_width: f32) -> f32 {
    if half_width <= 0.0 || distance >= half_width {
        return 0.0;
    }
    0.5 * (1.0 + (std::f32::consts::PI * distance / half_width).cos())
}

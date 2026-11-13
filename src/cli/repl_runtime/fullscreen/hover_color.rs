//! 悬停时提高前景对比度，浅色加深、深色提亮，不铺底。

/// 灰阶前景向白色混合的比例。
const LIFT: u16 = 45;
/// 有色相的前景（语法高亮等）向白色混合的比例，保留色相不被洗白。
const CHROMA_LIFT: u16 = 12;
/// 通道最大差超过该值视为有色相。
const CHROMA_THRESHOLD: u8 = 24;
/// 【终端】【默认悬停色】已知主题按背景强调，未知时沿用终端默认前景。
/// 参数：无；返回：不加粗、不铺底的前景 SGR。
pub(super) fn hover_default_fg() -> String {
    let Some((foreground, background)) = crate::render::terminal_palette::terminal_default_colors()
    else {
        return "\x1b[22;39m".into();
    };
    format!("\x1b[22;{}m", lifted_for_background(foreground, background))
}

const ANSI16: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (128, 0, 0),
    (0, 128, 0),
    (128, 128, 0),
    (0, 0, 128),
    (128, 0, 128),
    (0, 128, 128),
    (192, 192, 192),
    (128, 128, 128),
    (255, 0, 0),
    (0, 255, 0),
    (255, 255, 0),
    (0, 0, 255),
    (255, 0, 255),
    (0, 255, 255),
    (255, 255, 255),
];

/// 【全屏视图】【颜色提亮】把单通道向 255 混合，并保证至少抬一档。
///
/// 参数:
/// - `value`: 原通道
///
/// 返回:
/// - 提亮后的通道
#[cfg(test)]
pub(super) fn lift_channel(value: u8) -> u8 {
    let mixed = u16::from(value) + (255 - u16::from(value)) * LIFT / 100;
    mixed.max(u16::from(value).saturating_add(20)).min(255) as u8
}

/// 【全屏视图】【颜色提亮】有色相的通道只轻微向白色混合。
///
/// 参数:
/// - `value`: 原通道
///
/// 返回:
/// - 提亮后的通道
/// 把 256 色下标转成 RGB。
///
/// 参数:
/// - `index`: 0..=255
///
/// 返回:
/// - RGB
pub(super) fn color_256_rgb(index: u8) -> (u8, u8, u8) {
    if index < 16 {
        return ANSI16[usize::from(index)];
    }
    if index >= 232 {
        let value = 8 + (index - 232) * 10;
        return (value, value, value);
    }
    let cube = index - 16;
    let level = |part: u8| {
        if part == 0 {
            0
        } else {
            55 + part * 40
        }
    };
    (level(cube / 36), level((cube % 36) / 6), level(cube % 6))
}

/// 生成提亮后的真彩前景序列。
///
/// 参数:
/// - `red` / `green` / `blue`: 原色
///
/// 返回:
/// - `38;2;R;G;B` SGR
pub(super) fn lifted_truecolor(red: u8, green: u8, blue: u8) -> String {
    match crate::render::terminal_palette::terminal_default_colors() {
        Some((_, background)) => lifted_for_background((red, green, blue), background),
        None => format!("38;2;{red};{green};{blue}"),
    }
}

/// 【终端】【主题悬停色】色相轻强调，灰阶强强调，并保证正文对比度。
/// 参数：原始前景和实际背景；返回：真彩前景 SGR 参数。
fn lifted_for_background((red, green, blue): (u8, u8, u8), background: (u8, u8, u8)) -> String {
    let chroma = red.max(green).max(blue) - red.min(green).min(blue);
    // 1. 语法高亮等有色相的前景只轻抬一档，避免整块代码被洗成白色
    let amount = if chroma > CHROMA_THRESHOLD {
        CHROMA_LIFT
    } else {
        LIFT
    };
    let (red, green, blue) = crate::render::terminal_contrast::emphasize(
        (red, green, blue),
        background,
        f64::from(amount) / 100.0,
    );
    format!("38;2;{red};{green};{blue}")
}

/// 生成提亮后的 256 色前景序列。
///
/// 参数:
/// - `index`: 256 色下标
///
/// 返回:
/// - 真彩前景 SGR
pub(super) fn lifted_256(index: u8) -> String {
    if index < 16 {
        let code = if index < 8 {
            30 + index
        } else {
            90 + index - 8
        };
        return format!("22;{code}");
    }
    let (red, green, blue) = color_256_rgb(index);
    lifted_truecolor(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【浅色悬停回归】截图背景下的悬停前景必须保持可读对比度。
    #[test]
    fn light_background_hover_keeps_readable_contrast() {
        let output = lifted_for_background((137, 141, 145), (194, 213, 231));
        let values = output
            .trim_start_matches("38;2;")
            .split(';')
            .map(|part| part.parse::<u8>().unwrap())
            .collect::<Vec<_>>();
        let color = (values[0], values[1], values[2]);
        assert!(
            crate::render::terminal_contrast::contrast(color, (194, 213, 231)) >= 4.5,
            "{color:?}"
        );
    }

    /// 提亮后一定不比原值暗，已接近白色时顶到纯白。
    #[test]
    fn lift_moves_toward_white() {
        assert!(lift_channel(80) > 80);
        assert!(lift_channel(188) > 188);
        assert_eq!(lift_channel(240), 255);
        assert_eq!(lift_channel(255), 255);
    }

    /// 语法色保留色相：通道之间的差距不被抹平。
    #[test]
    fn chromatic_colors_keep_their_hue() {
        // 关键字紫色 (196,167,231)
        let lifted = lifted_for_background((196, 167, 231), (24, 24, 24));
        let parts: Vec<u8> = lifted
            .trim_start_matches("38;2;")
            .split(';')
            .map(|part| part.parse().unwrap())
            .collect();
        assert!(parts[2] - parts[1] > 40, "{lifted}");
        assert!(parts
            .iter()
            .zip([196u8, 167, 231])
            .all(|(new, old)| *new >= old));
    }
}

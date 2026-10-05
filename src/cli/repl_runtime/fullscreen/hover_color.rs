//! 悬停时把前景色向白色混合一档，避免加粗或铺底。

/// 向白色混合的比例。
const LIFT: u16 = 45;
/// 无显式前景时使用的悬停色：取消暗淡并提到纯白。
pub(super) const HOVER_DEFAULT_FG: &str = "\x1b[22;38;2;255;255;255m";

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
pub(super) fn lift_channel(value: u8) -> u8 {
    let mixed = u16::from(value) + (255 - u16::from(value)) * LIFT / 100;
    mixed.max(u16::from(value).saturating_add(20)).min(255) as u8
}

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
    format!(
        "38;2;{};{};{}",
        lift_channel(red),
        lift_channel(green),
        lift_channel(blue)
    )
}

/// 生成提亮后的 256 色前景序列。
///
/// 参数:
/// - `index`: 256 色下标
///
/// 返回:
/// - 真彩前景 SGR
pub(super) fn lifted_256(index: u8) -> String {
    let (red, green, blue) = color_256_rgb(index);
    lifted_truecolor(red, green, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 提亮后一定不比原值暗，已接近白色时顶到纯白。
    #[test]
    fn lift_moves_toward_white() {
        assert!(lift_channel(80) > 80);
        assert!(lift_channel(188) > 188);
        assert_eq!(lift_channel(240), 255);
        assert_eq!(lift_channel(255), 255);
    }
}

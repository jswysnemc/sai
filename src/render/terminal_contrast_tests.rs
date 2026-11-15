use super::*;

/// 【终端】【浅色回归】截图中的浅蓝背景及白底均保持正文可读。
#[test]
fn light_backgrounds_keep_secondary_and_accent_text_readable() {
    for background in [(194, 213, 231), (255, 255, 255), (228, 220, 210)] {
        for color in [
            (137, 141, 145),
            (103, 193, 207),
            (94, 196, 168),
            (188, 188, 188),
        ] {
            let readable = readable_color_on(color, background, MIN_TEXT_CONTRAST);
            assert!(
                contrast(readable, background) >= MIN_TEXT_CONTRAST,
                "{color:?} -> {readable:?} on {background:?}"
            );
        }
    }
}

/// 【终端】【对比度回归】已可读颜色保持原值，黑白对比度满足标准；无参数或返回值。
#[test]
fn readable_colors_stay_unchanged_and_black_white_contrast_is_maximal() {
    assert_eq!(
        readable_color_on((38, 40, 42), (255, 255, 255), MIN_TEXT_CONTRAST),
        (38, 40, 42)
    );
    assert!((contrast((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 0.001);
}

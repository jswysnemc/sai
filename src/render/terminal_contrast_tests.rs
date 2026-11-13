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

/// 【终端】【主题回归】浅色悬停加深、深色悬停提亮，已可读颜色不被无故改写。
#[test]
fn emphasis_follows_background_and_readable_colors_stay_unchanged() {
    let gray = (137, 141, 145);
    assert!(emphasize(gray, (194, 213, 231), 0.45).0 < gray.0);
    assert!(emphasize(gray, (28, 30, 34), 0.45).0 > gray.0);
    assert_eq!(
        readable_color_on((38, 40, 42), (255, 255, 255), MIN_TEXT_CONTRAST),
        (38, 40, 42)
    );
    assert!((contrast((0, 0, 0), (255, 255, 255)) - 21.0).abs() < 0.001);
}

use super::*;

/// 【终端】【文字样式回归】浅色背景取消暗淡，并保留文字、图片和光标控制。
#[test]
fn light_theme_corrects_dim_without_touching_content_or_graphics() {
    let graphics = "\x1b_Ga=p,i=1;data\x1b\\";
    let source = format!("\x1b[5;2H\x1b[2;36mThinking text\x1b[0m{graphics}");
    let output = correct_ansi_text(&source, Some(((38, 40, 42), (194, 213, 231))), true);
    assert!(output.contains("Thinking text"));
    assert!(output.contains(graphics));
    assert!(output.contains("\x1b[5;2H"));
    assert!(!output.contains("[2;36m"));
    assert!(output.contains("[22m"));
    assert!(output.contains("[38;2;"));
}

/// 【终端】【显式底色回归】深色输入框上的可读白字不按外层浅色背景加深。
#[test]
fn explicit_dark_background_keeps_white_text_and_unknown_theme_keeps_colors() {
    let source = "\x1b[48;5;235m\x1b[38;5;252mInput\x1b[0m";
    let corrected = correct_ansi_text(source, Some(((38, 40, 42), (194, 213, 231))), true);
    assert!(corrected.contains("\x1b[38;5;252mInput"));
    assert_eq!(crate::render::ansi_clip::strip_ansi(&corrected), "Input");
    let source = "\x1b[38;2;103;193;207mCode\x1b[0m";
    assert_eq!(correct_ansi_text(source, None, true), source);
    assert_eq!(
        correct_ansi_text("\x1b[2mNote", None, false),
        "\x1b[22mNote"
    );
}

/// 【终端】【256 色回归】不支持真彩时使用可读 256 色，不强行输出真彩序列。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn limited_color_terminals_use_readable_palette_colors() {
    let output = correct_ansi_text("\x1b[2mNote", Some(((38, 40, 42), (255, 255, 255))), false);
    assert!(output.contains("[38;5;"));
    assert!(!output.contains("[38;2;"));
}

/// 【终端】【解析真彩】提取最后一个真彩前景色序列的 RGB 分量。
///
/// 参数:
/// - `rendered`: 已校正的 ANSI 文本
///
/// 返回:
/// - 最后一个 `\x1b[38;2;R;G;Bm` 对应的 RGB 值
fn last_rgb_foreground(rendered: &str) -> Rgb {
    let segment = rendered
        .rsplit("\x1b[38;2;")
        .next()
        .expect("missing truecolor foreground");
    let raw = segment.split('m').next().expect("missing SGR terminator");
    let mut parts = raw.split(';').map(|part| part.parse::<u8>().unwrap());
    (
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    )
}

/// 【终端】【色相保留回归】`dim + ANSI 16 色` 在深浅主题下均保留青色与红色色相及对比度。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn dim_ansi_16_colors_preserve_cyan_and_red_hue_with_readable_contrast() {
    for defaults in [
        ((214, 218, 222), (24, 28, 32)),
        ((38, 40, 42), (248, 249, 250)),
    ] {
        // 1. 【终端】【青色保留】思考块与折叠提示的分步或合并 SGR 仍呈现青色色相
        for cyan_input in [
            "\x1b[2m\x1b[36m◇ Thought\x1b[0m",
            "\x1b[2;36m▸ 6 lines hidden\x1b[0m",
            "\x1b[2;96mBright cyan\x1b[0m",
        ] {
            let corrected = correct_ansi_text(cyan_input, Some(defaults), true);
            let before_reset = corrected.split("\x1b[0m").next().unwrap();
            let rgb = last_rgb_foreground(before_reset);
            assert!(
                rgb.2 > rgb.0 && rgb.1 > rgb.0,
                "expected cyan hue on {defaults:?}, got {rgb:?} in {corrected:?}"
            );
            assert!(
                super::super::terminal_contrast::contrast(rgb, defaults.1)
                    >= MIN_TEXT_CONTRAST - 0.05,
                "insufficient contrast for {rgb:?} on {:?}",
                defaults.1
            );
        }

        // 2. 【终端】【红色保留】先设红色再设 dim 的错误详情仍呈现红色色相
        for red_input in [
            "\x1b[31m\x1b[2m错误详情续行\x1b[0m",
            "\x1b[2;31mDim red\x1b[0m",
            "\x1b[91m\x1b[2mBright red detail\x1b[0m",
        ] {
            let corrected = correct_ansi_text(red_input, Some(defaults), true);
            let before_reset = corrected.split("\x1b[0m").next().unwrap();
            let rgb = last_rgb_foreground(before_reset);
            assert!(
                rgb.0 > rgb.1 && rgb.0 > rgb.2,
                "expected red hue on {defaults:?}, got {rgb:?} in {corrected:?}"
            );
            assert!(
                super::super::terminal_contrast::contrast(rgb, defaults.1)
                    >= MIN_TEXT_CONTRAST - 0.05,
                "insufficient contrast for {rgb:?} on {:?}",
                defaults.1
            );
        }

        // 3. 【终端】【默认重置】仅 SGR 39 清空前景色相，不残留上一条 ANSI 色相
        let reset_fg = correct_ansi_text("\x1b[31mRed\x1b[39m\x1b[2mNeutral\x1b[0m", Some(defaults), true);
        let neutral_part = reset_fg.split("Red").nth(1).unwrap().split("Neutral").next().unwrap();
        let neutral_rgb = last_rgb_foreground(neutral_part);
        assert!(
            neutral_rgb.0.abs_diff(neutral_rgb.1) <= 6,
            "SGR 39 should reset to neutral default foreground, got {neutral_rgb:?}"
        );
    }
}

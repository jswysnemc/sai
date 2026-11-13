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
#[test]
fn limited_color_terminals_use_readable_palette_colors() {
    let output = correct_ansi_text("\x1b[2mNote", Some(((38, 40, 42), (255, 255, 255))), false);
    assert!(output.contains("[38;5;"));
    assert!(!output.contains("[38;2;"));
}

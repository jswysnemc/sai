use super::super::{chrome_input_pad_row, chrome_input_row, ChromeInputPrefix};
use crate::cli::repl_text::visible_width;

/// 【终端】【输入回滚测试】共用输入框在可容纳提示符的终端内不越界，并保留背景留白。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_input_stays_within_terminal_width() {
    for cols in [4, 8, 40, 80, 120] {
        for prefix in [
            ChromeInputPrefix::Message,
            ChromeInputPrefix::Shell,
            ChromeInputPrefix::Continuation,
        ] {
            let row = chrome_input_row(prefix, "\x1b[1m中文 wide input\x1b[0m", cols);
            assert!(visible_width(&row) <= cols, "width {cols}: {row:?}");
            assert!(row.contains("\x1b[48;5;235m"));
        }
        assert_eq!(visible_width(&chrome_input_pad_row(cols)), cols);
    }
}

/// 【终端】【输入回滚测试】全屏与内联界面共用相同背景、提示符、留白与宽字符布局。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_and_inline_share_input_geometry() {
    use crate::config::TuiMode;
    use crate::render::tui_mode;
    for cols in [4, 8, 40, 80, 120] {
        for prefix in [
            ChromeInputPrefix::Message,
            ChromeInputPrefix::Shell,
            ChromeInputPrefix::Continuation,
        ] {
            tui_mode::set_override(Some(TuiMode::Inline));
            let inline = chrome_input_row(prefix, "hello 中文", cols);
            let padding = chrome_input_pad_row(cols);
            tui_mode::set_override(Some(TuiMode::Fullscreen));
            assert_eq!(chrome_input_row(prefix, "hello 中文", cols), inline);
            assert_eq!(chrome_input_pad_row(cols), padding);
        }
    }
    tui_mode::set_override(None);
}

use super::super::{chrome_input_pad_row, chrome_input_row, ChromeInputPrefix};
use crate::cli::repl_text::visible_width;

/// 【终端】【输入回滚测试】共用输入框在可容纳提示符的终端内不越界，并保留背景留白。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_input_stays_within_terminal_width() {
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

/// 【终端】【输入回滚测试】两种样式共用相同背景、提示符、留白与宽字符布局。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_and_classic_share_input_geometry() {
    use crate::config::TuiStyle;
    use crate::render::tui_style;
    for cols in [4, 8, 40, 80, 120] {
        for prefix in [
            ChromeInputPrefix::Message,
            ChromeInputPrefix::Shell,
            ChromeInputPrefix::Continuation,
        ] {
            tui_style::set_override(Some(TuiStyle::Classic));
            let classic = chrome_input_row(prefix, "hello 中文", cols);
            let padding = chrome_input_pad_row(cols);
            tui_style::set_override(Some(TuiStyle::Deepseek));
            assert_eq!(chrome_input_row(prefix, "hello 中文", cols), classic);
            assert_eq!(chrome_input_pad_row(cols), padding);
        }
    }
    tui_style::set_override(None);
}

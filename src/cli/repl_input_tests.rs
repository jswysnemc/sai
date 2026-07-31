use super::args::Cli;
use super::chat::drain_stdin;
use super::input_flags::parse_message_input_flags;
use super::repl_input::{repl_history_is_clean, repl_should_browse_history};
use super::repl_input_render::*;
use super::repl_text::*;
use super::*;
use clap::Parser;

#[test]
fn prompt_rows_wrap_at_terminal_width() {
    assert_eq!(repl_prompt_rows_for_cols("", &["1234567".into()], 10), 1);
    assert_eq!(repl_prompt_rows_for_cols("", &["1234567890".into()], 10), 2);
    assert_eq!(
        repl_prompt_rows_for_cols("", &["123".into(), "456".into()], 10),
        2
    );
}

#[test]
fn cursor_position_wraps_at_terminal_width() {
    assert_eq!(repl_cursor_position_for_cols("", "1234567", 7, 10), (7, 0));
    assert_eq!(
        repl_cursor_position_for_cols("", "1234567890", 10, 10),
        (0, 1)
    );
    assert_eq!(repl_cursor_position_for_cols("", "123\n456", 7, 10), (3, 1));
    assert_eq!(repl_cursor_position_for_cols("", "1234567", 3, 10), (3, 0));
}

#[test]
fn wide_chars_wrap_as_whole_units() {
    // 第 5 个宽字符在第 9 列放不下，整字符移到下一行并在行尾留空列：
    // 纯模运算会把光标算在第 3 列，真实终端在第 4 列
    assert_eq!(
        repl_cursor_position_for_cols("", "a字字字字字字", 7, 10),
        (4, 1)
    );
    assert_eq!(
        repl_prompt_rows_for_cols("", &["a字字字字字字".into()], 10),
        2
    );
}

#[test]
fn combining_marks_do_not_advance_cursor() {
    // 组合变音符宽度为 0，不推进光标列
    assert_eq!(visible_width("e\u{0301}"), 1);
    assert_eq!(
        repl_cursor_position_for_cols("", "e\u{0301}x", 3, 10),
        (2, 0)
    );
}

#[test]
fn tabs_advance_to_next_tab_stop() {
    // 制表符前进到下一个 8 列制表位，宽度不再按 1 计
    assert_eq!(repl_cursor_position_for_cols("", "\tx", 2, 20), (9, 0));
}

#[test]
fn strips_osc_sequences_without_residue() {
    // OSC 标题序列整段清除，负载不残留进输入
    assert_eq!(strip_terminal_control_sequences("a\x1b]0;title\x07b"), "ab");
    assert_eq!(strip_terminal_control_sequences("a\x1bOPb"), "ab");
}

#[test]
fn cli_parses_trailing_clipboard_flag_as_message_part() {
    let cli = Cli::try_parse_from(["sai", "总结", "-c"]).unwrap();
    let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
    assert!(!input.clipb);
    assert_eq!(input.message, "总结 -c");
}

#[test]
fn cli_parses_leading_clipboard_flag_as_option() {
    let cli = Cli::try_parse_from(["sai", "-c", "总结"]).unwrap();
    let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
    assert!(input.clipb);
    assert_eq!(input.message, "总结");
}

#[test]
fn cli_parses_leading_web_search_flag_as_option() {
    let cli = Cli::try_parse_from(["sai", "-w", "搜索"]).unwrap();
    let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
    assert!(input.web_search);
    assert_eq!(input.message, "搜索");
}

#[test]
fn cli_parses_explain_with_clipboard_and_web_flags() {
    let cli = Cli::try_parse_from(["sai", "-e", "-c", "-w", "解释这段命令"]).unwrap();

    assert!(cli.explain);
    assert!(cli.clipb);
    assert!(cli.web_search);
    assert_eq!(cli.message, ["解释这段命令"]);
}

#[test]
fn cli_parses_explain_without_instruction() {
    let cli = Cli::try_parse_from(["sai", "-e"]).unwrap();

    assert!(cli.explain);
    assert!(cli.message.is_empty());
}

#[test]
fn shell_intercept_parses_leading_clipboard_flag_after_separator() {
    let cli = Cli::try_parse_from([
        "sai",
        "--shell-intercept",
        "--shell",
        "zsh",
        "--",
        "-c",
        "总结",
    ])
    .unwrap();
    let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
    assert!(input.clipb);
    assert_eq!(input.message, "总结");
}

#[test]
fn shell_intercept_parses_trailing_clipboard_flag_after_separator() {
    let cli = Cli::try_parse_from([
        "sai",
        "--shell-intercept",
        "--shell",
        "zsh",
        "--",
        "总结",
        "-c",
    ])
    .unwrap();
    let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
    assert!(!input.clipb);
    assert_eq!(input.message, "总结 -c");
}

#[test]
fn shell_intercept_parses_leading_web_search_flag_after_separator() {
    let cli = Cli::try_parse_from([
        "sai",
        "--shell-intercept",
        "--shell",
        "zsh",
        "--",
        "-w",
        "搜索",
    ])
    .unwrap();
    let input = parse_message_input_flags(cli.message, cli.clipb, cli.web_search);
    assert!(input.web_search);
    assert_eq!(input.message, "搜索");
}

#[test]
fn drain_stdin_does_not_panic() {
    drain_stdin();
}

#[test]
fn input_helpers_edit_at_cursor() {
    let mut input = "abcd".to_string();
    let mut cursor = 2;
    insert_char_at_cursor(&mut input, &mut cursor, '中');
    assert_eq!(input, "ab中cd");
    assert_eq!(cursor, 3);

    remove_char_before_cursor(&mut input, &mut cursor);
    assert_eq!(input, "abcd");
    assert_eq!(cursor, 2);

    remove_char_at_cursor(&mut input, cursor);
    assert_eq!(input, "abd");
    assert_eq!(cursor, 2);
}

#[test]
fn input_helpers_remove_word_before_cursor() {
    let mut input = "hello world  ".to_string();
    let mut cursor = input.chars().count();
    remove_word_before_cursor(&mut input, &mut cursor);
    assert_eq!(input, "hello ");
    assert_eq!(cursor, 6);

    let mut input = "前面 中间 后面".to_string();
    let mut cursor = 6;
    remove_word_before_cursor(&mut input, &mut cursor);
    assert_eq!(input, "前面 后面");
    assert_eq!(cursor, 3);
}

#[test]
fn input_helpers_insert_paste_at_cursor() {
    let mut input = "前后".to_string();
    let mut cursor = 1;
    insert_str_at_cursor(&mut input, &mut cursor, "中间");
    assert_eq!(input, "前中间后");
    assert_eq!(cursor, 3);
}

#[test]
fn input_helpers_insert_newline_at_cursor() {
    let mut input = "前后".to_string();
    let mut cursor = 1;
    insert_newline_at_cursor(&mut input, &mut cursor);
    assert_eq!(input, "前\n后");
    assert_eq!(cursor, 2);
}

#[test]
fn slash_can_be_inserted_into_fresh_composer_after_a_turn() {
    let mut input = String::new();
    let mut cursor = 0;

    insert_char_at_cursor(&mut input, &mut cursor, '/');

    assert_eq!(input, "/");
    assert_eq!(cursor, 1);
    assert!(!repl_command_suggestions(&input).is_empty());
}

#[test]
fn history_browsing_does_not_replace_unsubmitted_draft() {
    let history = vec!["first".to_string(), "second".to_string()];

    assert!(repl_should_browse_history("", &history, None));
    assert!(repl_should_browse_history("second", &history, Some(1)));
    assert!(repl_history_is_clean("second", &history, Some(1)));
    assert!(!repl_should_browse_history("draft", &history, None));
    assert!(!repl_should_browse_history(
        "second edited",
        &history,
        Some(1)
    ));
}

#[test]
fn long_paste_visible_lines_are_collapsed() {
    let lines = (0..20)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>();
    let visible = repl_visible_input_lines("[YOLO] > ", &lines, 12, true);

    assert!(visible.collapsed);
    assert_eq!(visible.lines.len(), 3);
    assert_eq!(visible.lines[0], "line 0");
    assert!(visible.lines[1].contains("18") || visible.lines[1].contains("已隐藏 18"));
    assert_eq!(visible.lines[2], "line 19");
    assert_eq!(lines.len(), 20);
}

#[test]
fn manual_multiline_input_collapses_when_over_visible_rows() {
    // 非粘贴输入超过可见行上限时同样收缩，否则 composer 会被顶出屏幕
    let lines = (0..20)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>();
    let visible = repl_visible_input_lines("", &lines, 12, false);

    assert!(visible.collapsed);
    assert_eq!(visible.lines.len(), 3);
}

#[test]
fn three_line_input_is_not_misdetected_as_collapsed() {
    // 恰好 3 行且未超限：显示行数与折叠输出长度相同，标志必须为未折叠
    let lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let visible = repl_visible_input_lines("", &lines, 12, false);

    assert!(!visible.collapsed);
    assert_eq!(visible.lines, lines);
}

#[test]
fn long_single_line_paste_visible_is_collapsed() {
    let lines = vec!["x".repeat(400)];
    let visible = repl_visible_input_lines("", &lines, 12, true);
    assert!(visible.collapsed);
    assert_eq!(visible.lines.len(), 2);
    assert!(visible.lines[0].ends_with('…'));
    assert!(visible.lines[1].contains("hidden") || visible.lines[1].contains("已隐藏"));
}

#[test]
fn strips_terminal_control_sequences_from_repl_text() {
    assert_eq!(
        strip_terminal_control_sequences("\x1b[E表情包\x1b[0m\x07 ok"),
        "表情包 ok"
    );
    assert_eq!(
        strip_terminal_control_sequences("line1\nline2\tend"),
        "line1\nline2\tend"
    );
}

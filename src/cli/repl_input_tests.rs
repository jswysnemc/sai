use super::args::Cli;
use super::chat::drain_stdin;
use super::input_flags::parse_message_input_flags;
use super::repl::load_repl_input_history;
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

    assert_eq!(visible.len(), 3);
    assert_eq!(visible[0], "line 0");
    assert!(visible[1].contains("18") || visible[1].contains("已隐藏 18"));
    assert_eq!(visible[2], "line 19");
    assert_eq!(lines.len(), 20);
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

#[test]
fn repl_history_loads_user_messages_from_state() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths {
        config_dir: PathBuf::new(),
        config_file: PathBuf::new(),
        secrets_file: PathBuf::new(),
        skills_dir: PathBuf::new(),
        data_dir: PathBuf::new(),
        cache_dir: PathBuf::new(),
        state_dir: temp.path().to_path_buf(),
        pictures_dir: PathBuf::new(),
        fish_hook_file: PathBuf::new(),
        bash_hook_file: PathBuf::new(),
        zsh_hook_file: PathBuf::new(),
        powershell_hook_file: PathBuf::new(),
    };
    let state = StateStore::new(&paths).unwrap();
    state.append_message("user", "first").unwrap();
    state.append_assistant_message("reply", None).unwrap();
    state.append_message("user", "\x1b[Esecond").unwrap();

    assert_eq!(
        load_repl_input_history(&state).unwrap(),
        vec!["first".to_string(), "second".to_string()]
    );
}

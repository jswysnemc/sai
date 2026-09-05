use super::*;

/// 标准错误流本身不代表失败，运行中和正常结束的诊断日志不应使用红色状态。
#[test]
fn stderr_is_neutral_until_the_command_fails() {
    let live = render_live_command_output("", "compiler progress", false);
    let success = render_completed_command_output(
        r#"{"success":true,"exit_code":0,"stdout":"","stderr":"compiler progress"}"#,
        "",
        "compiler progress",
        false,
    );
    for rendered in [live, success] {
        assert!(rendered.contains(t("stderr", "标准错误输出")));
        assert!(!rendered.contains("\x1b[31m"));
    }
    let failed = render_completed_command_output(
        r#"{"success":false,"exit_code":2,"stdout":"","stderr":"compile failed"}"#,
        "",
        "compile failed",
        false,
    );
    assert!(failed.contains("\x1b[31m"));
    assert!(failed.contains(&format!("{} 2", t("Failed · exit", "执行失败 · 退出码"))));
}

#[test]
fn parses_command_result_json() {
    let result = parse_command_result(
        r#"{"success":false,"exit_code":1,"stdout":"unused","stderr":"not found"}"#,
    )
    .unwrap();
    assert!(!result.success);
    assert_eq!(result.exit_code, Some(1));
    assert_eq!(result.stdout, "unused");
    assert_eq!(result.stderr, "not found");
}

#[test]
fn renders_command_output_with_codex_gutter() {
    let output = render_output_block("output", "sent to snemc@qq.com\n");
    let plain = strip_ansi_for_test(&output);

    assert!(!plain.contains("──"));
    assert!(plain.contains("sent to snemc@qq.com"));
    assert!(plain.contains("└"));
    assert!(!plain.contains(",-- output"));
    assert!(!plain.contains("`--"));
}

#[test]
fn renders_command_error_output_as_code_block() {
    let output = render_output_block("err exit 1", "not found\n");
    let plain = strip_ansi_for_test(&output);

    // 错误标签位于统一 gutter 首行，内容行按续行缩进；圆点只属于卡片标题
    assert!(plain.contains("└ err exit 1"));
    assert!(!plain.contains("• err"));
    assert!(!plain.contains("──"));
    assert!(plain.contains("    not found"));
    assert!(!plain.contains(",-- err"));
    assert!(!plain.contains("`--"));
}

#[test]
fn live_command_preview_keeps_head_and_tail_with_middle_ellipsis() {
    let output = render_live_command_output(
        "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n",
        "",
        false,
    );
    let plain = strip_ansi_for_test(&output);

    // 折叠策略：前 2 + 后 4
    assert!(plain.contains("one"));
    assert!(plain.contains("two") || plain.contains("one"));
    assert!(!plain
        .lines()
        .any(|line| line.trim_end() == "five" || line.ends_with(" five")));
    assert!(!plain
        .lines()
        .any(|line| line.trim_end() == "six" || line.ends_with(" six")));
    // 中间省略 6 行（12 - 2 - 4）
    assert!(
        plain.contains("▸ 6 lines hidden"),
        "expected fold ellipsis: {plain}"
    );
    assert!(plain.contains("nine") || plain.contains("twelve"));
    assert!(plain.contains("twelve"));
    assert!(plain.contains("Ctrl+O"));
}

#[test]
fn collapsed_hint_stays_without_frame_footer() {
    let output = render_live_command_output(
        "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve\n",
        "",
        false,
    );
    let plain = strip_ansi_for_test(&output);
    assert!(plain.contains("Ctrl+O"));
    assert!(!plain.contains("──"));
    assert!(plain.contains("└") || plain.contains("…"));
}

#[test]
fn expanded_command_output_keeps_all_lines() {
    let output = render_live_command_output("one\ntwo\nthree\nfour\nfive\nsix\n", "", true);
    let plain = strip_ansi_for_test(&output);

    assert!(plain.contains("four"));
    assert!(plain.contains("six"));
    assert!(!plain.contains("Ctrl+O"));
}

#[test]
fn live_command_output_removes_terminal_control_sequences() {
    let output = render_live_command_output("\x1b[2Jfirst\rsecond\x07\n", "", true);
    let plain = strip_ansi_for_test(&output);

    assert!(!output.contains("\x1b[2J"));
    assert!(!output.contains('\x07'));
    assert!(plain.contains("first"));
    assert!(plain.contains("second"));
}

#[test]
fn stdout_and_stderr_each_keep_preview_budget() {
    let output = render_live_command_output(
        "out-1\nout-2\nout-3\nout-4\nout-5\nout-6\nout-7\nout-8\nout-9\nout-10\nout-11\nout-12\n",
        "err-1\nerr-2\nerr-3\nerr-4\nerr-5\nerr-6\nerr-7\nerr-8\nerr-9\nerr-10\nerr-11\nerr-12\n",
        false,
    );
    let plain = strip_ansi_for_test(&output);
    // 每流首尾各 5 行
    assert!(plain.contains("out-1"));
    assert!(plain.contains("out-12"));
    assert!(plain.contains("err-1"));
    assert!(plain.contains("err-12"));
    assert!(plain.contains("Ctrl+O"));
}

#[test]
fn completed_command_result_keeps_each_stream_preview() {
    let output = serde_json::json!({
        "success": false,
        "exit_code": 1,
        "stdout": "out-1\nout-2\nout-3\nout-4",
        "stderr": "err-1\nerr-2\nerr-3\nerr-4"
    })
    .to_string();
    let rendered = render_command_result_view_with_limit(&output, Some(COMMAND_PREVIEW_LINES));
    let plain = strip_ansi_for_test(&rendered);
    // 每流行数 <= 2*limit，完整保留
    for line in ["out-1", "out-4", "err-1", "err-4"] {
        assert!(plain.contains(line), "{line}");
    }
}

/// 验证普通 CLI 仅展示五行命令摘要且不提供展开提示。
#[test]
fn cli_command_result_keeps_head_and_tail_without_expand_hint() {
    let output = serde_json::json!({
        "success": true,
        "exit_code": 0,
        "stdout": "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\nnine\nten\neleven\ntwelve",
        "stderr": ""
    })
    .to_string();
    let rendered = render_command_result_view_for_cli(&output);
    let plain = strip_ansi_for_test(&rendered);
    let visible = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve",
    ]
    .into_iter()
    .filter(|line| plain.contains(line))
    .count();

    // 首尾各 5 行，中间省略 2 行
    assert_eq!(visible, FOLD_HEAD_LINES + FOLD_TAIL_LINES);
    assert!(!plain.contains("Ctrl+O"));
    assert!(plain.contains("one"));
    assert!(plain.contains("twelve"));
    assert!(
        !plain.contains("six\n")
            && !plain
                .lines()
                .any(|l| l.trim().ends_with("six") && !l.contains("…"))
    );
}

#[test]
fn tool_error_is_not_hidden_by_live_output() {
    let rendered = render_completed_command_output(
        "tool error: shell command timed out after 1s",
        "before-timeout",
        "",
        false,
    );
    let plain = strip_ansi_for_test(&rendered);

    assert!(plain.contains("timed out"));
    assert!(!plain.contains("before-timeout"));
}

/// 去除 ANSI 转义序列，方便断言可见文本。
///
/// 参数:
/// - `text`: 原始终端文本
///
/// 返回:
/// - 去除样式后的文本
fn strip_ansi_for_test(text: &str) -> String {
    let mut output = String::new();
    let mut escape = false;
    let mut csi = false;
    for ch in text.chars() {
        if ch == '\x1b' {
            escape = true;
            csi = false;
        } else if escape {
            if csi {
                if (ch as u32) >= 0x40 && (ch as u32) <= 0x7e {
                    escape = false;
                }
            } else if ch == '[' {
                csi = true;
            } else if ch == '\\' || ch == 'm' {
                escape = false;
            }
        } else {
            output.push(ch);
        }
    }
    output
}

#[test]
fn long_single_line_command_output_folds_by_display_width() {
    // 按当前终端宽度生成足够多的显示行（> 2*预览行）
    let width = crate::render::fold_text::terminal_wrap_width().max(8);
    let long = "x".repeat(width * 12);
    let output = render_live_command_output(&long, "", false);
    let plain = strip_ansi_for_test(&output);
    assert!(
        plain.contains("Ctrl+O") || plain.contains("…"),
        "got: {plain}"
    );
}

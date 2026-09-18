use super::*;

/// 构造测试用底栏 chrome。
///
/// 返回:
/// - chrome 状态
fn test_chrome() -> ReplChrome {
    ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 200_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
        status_plugin: None,
    }
}

/// 角色标记常驻底栏，且排在活动提示之前。
#[test]
fn role_badge_sits_at_the_left_of_the_footer() {
    let mut chrome = test_chrome();
    assert!(!chrome.footer_line(60).contains("跟随中"));

    chrome.set_role_badge(Some("跟随中".to_string()));
    assert!(chrome.footer_line(60).contains("跟随中"));
    // 活动提示（Ctrl+C 停止）出现时角色标记仍在
    let busy = chrome.footer_line_with_activity(60, Some("Ctrl+C"));
    assert!(busy.contains("跟随中"));
    assert!(busy.contains("Ctrl+C"));
    assert!(busy.find("跟随中").unwrap() < busy.find("Ctrl+C").unwrap());
}

/// 【TUI】【实时用量】验证实报读数覆盖上下文占比并带出缓存命中。
#[test]
fn live_usage_overrides_context_status() {
    let mut chrome = test_chrome();
    assert_eq!(chrome.context_status(), "0.0%/200k");

    chrome.apply_live_usage(Some(50_000), Some(0.96));

    assert_eq!(chrome.context_status(), "25.0%/200k cache 96%");
}

/// 【TUI】【实时用量】验证轮次尚无实报读数时保留原快照显示。
#[test]
fn live_usage_without_reading_keeps_snapshot() {
    let mut chrome = test_chrome();
    chrome.context_ratio = 0.141;

    chrome.apply_live_usage(None, None);

    assert_eq!(chrome.context_status(), "14.1%/200k");
}

/// 【TUI】【底栏路径】验证家目录压缩为 ~ 且只认目录边界。
#[test]
fn home_prefix_is_compressed_on_directory_boundary() {
    let sep = std::path::MAIN_SEPARATOR;
    let home = format!("{sep}home{sep}snemc");

    assert_eq!(compress_with_home(&home, &home), "~");
    assert_eq!(
        compress_with_home(&format!("{home}{sep}workspace{sep}sai"), &home),
        format!("~{sep}workspace{sep}sai")
    );
    // 同名前缀的兄弟目录不能被压缩
    assert_eq!(
        compress_with_home(&format!("{home}-backup"), &home),
        format!("{home}-backup")
    );
    assert_eq!(
        compress_with_home(&format!("{sep}etc{sep}sai"), &home),
        format!("{sep}etc{sep}sai")
    );
}

#[test]
fn status_line_keeps_left_and_right() {
    let line = chrome_status_line("0.0%/272k (auto)", "gpt · xhigh", 40);
    assert!(line.contains("0.0%/272k (auto)"));
    assert!(line.contains("gpt · xhigh"));
}

/// 【TUI】【底栏】会话标题不再进底栏，右侧只剩工作目录。
#[test]
fn footer_keeps_only_the_directory_on_the_right() {
    let chrome = test_chrome();
    let line = chrome.footer_line(80);
    let plain = strip_ansi(&line);
    let trimmed = plain.trim_end();
    assert!(
        trimmed.ends_with("/workspace"),
        "右侧最后一段必须是目录: {plain}"
    );
    // 目录与左侧状态之间只允许 gap 空白，不允许再混入标题等其它段落
    let directory_at = trimmed.len() - "/workspace".len();
    assert!(
        plain[..directory_at].trim_end().ends_with("auto"),
        "左侧状态与目录之间不应有其它段落: {plain}"
    );
}

#[test]
fn footer_puts_activity_before_mode() {
    let chrome = test_chrome();
    let line = chrome.footer_line_with_activity(80, Some("Working 12s"));
    let plain = crate::render::activity_animation::strip_ansi_for_test(&line);
    let work = plain.find("Working 12s").expect("activity");
    let mode = plain.find("yolo").expect("mode");
    assert!(work < mode, "{plain}");
}

#[test]
fn footer_puts_mode_before_context() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 272_000,
        model: "gpt".to_string(),
        thinking: "xhigh".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
        status_plugin: None,
    };
    let line = chrome.footer_line(80);
    let plain = strip_ansi(&line);
    assert!(
        plain.starts_with(' '),
        "footer must keep left outer pad: {plain:?}"
    );
    assert!(
        plain.ends_with(' '),
        "footer must keep right outer pad: {plain:?}"
    );
    assert!(plain.contains("yolo"));
    assert!(plain.contains("0.0%/272k"));
    assert!(plain.contains("gpt"));
    assert!(plain.contains("xhigh"));
    assert!(plain.contains("/workspace"));
    assert!(!plain.contains("main"));
    assert!(line.contains("\x1b[38;5;110m"));
    assert!(line.contains("\x1b[38;5;109m"));
    assert_eq!(visible_width(&line), 80);
}

#[test]
fn footer_line_never_exceeds_terminal_cols() {
    let chrome = ReplChrome {
        mode: AgentMode::AutoAudit,
        context_ratio: 0.12,
        context_window_tokens: 500_000,
        model: "gpt-5.6-sol".to_string(),
        thinking: "auto".to_string(),
        directory: "/home/snemc/workspace/sai/very/long/path/segment".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
        status_plugin: None,
    };
    for cols in [20usize, 40, 59, 60, 80, 120] {
        let line = chrome.footer_line(cols);
        let width = visible_width(&line);
        assert!(
            width <= cols,
            "cols={cols} width={width} plain={}",
            strip_ansi(&line)
        );
    }
}

#[test]
fn fit_status_segments_avoids_forced_gap_overflow() {
    let left = "yolo  0.0%/500k  gpt-5.6-sol  auto";
    let right = "/home/snemc/workspace/sai";
    let cols = visible_width(left) + visible_width(right);
    let (fitted_left, fitted_right, gap) = fit_status_segments(left, right, cols);
    assert_eq!(gap, 0);
    assert_eq!(
        visible_width(&fitted_left) + gap + visible_width(&fitted_right),
        cols
    );
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut escape = false;
    for ch in text.chars() {
        if ch == '\x1b' {
            escape = true;
            continue;
        }
        if escape {
            if ch == 'm' {
                escape = false;
            }
            continue;
        }
        out.push(ch);
    }
    out
}

#[test]
fn format_token_k_scales_thousands() {
    assert_eq!(format_token_k(272_000), "272k");
    assert_eq!(format_token_k(1_500), "1.5k");
    assert_eq!(format_token_k(42), "42");
}

#[test]
fn input_row_prefix_follows_line_role() {
    let first = chrome_input_row(ChromeInputPrefix::Message, "hello", 20);
    assert!(first.contains(CHROME_PANEL_BG));
    assert!(first.contains('→'));
    assert!(first.contains("hello"));
    // shell 模式换 $ 提示符
    let shell = chrome_input_row(ChromeInputPrefix::Shell, "!ls", 20);
    assert!(shell.contains('$'));
    assert!(!shell.contains('→'));
    // 续行只保留缩进
    let cont = chrome_input_row(ChromeInputPrefix::Continuation, "wrapped", 20);
    assert!(!cont.contains('→') && !cont.contains('$'));
    assert!(cont.contains("wrapped"));
    // 输入上方一行空白
    assert_eq!(chrome_fixed_rows(), 1);
}

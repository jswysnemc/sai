use super::ComposerFrame;
use crate::agent::AgentMode;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::viewport::{InlineViewport, TerminalSize};

/// 验证 composer 在固定 viewport 内写入底部，并将光标放回输入行。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn draws_at_viewport_bottom_and_restores_input_cursor() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, "hello".to_string(), 5, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 40, rows: 12 }, frame.height(40), 8);
    let mut output = Vec::new();

    frame.draw_lines(&mut output, &viewport, None).unwrap();

    let output = String::from_utf8(output).unwrap();
    // 极简输入行：无彩条、无背景，只保留输入文本
    assert!(output.contains("hello"));
    assert!(!output.contains('▏'));
    assert!(output.contains("\x1b[48;5;235m"));
}

/// 验证重绘期间先隐藏光标、结束时在输入位置恢复显示。
///
/// 清行与逐行打印会带着光标扫过整个 composer 区域，
/// 可见状态下表现为光标在输入框与面板/状态行之间来回跳动。
#[test]
fn repaint_hides_cursor_first_and_shows_it_last() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, "hello".to_string(), 5, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 40, rows: 12 }, frame.height(40), 8);
    let mut output = Vec::new();

    frame.draw_lines(&mut output, &viewport, None).unwrap();

    let output = String::from_utf8(output).unwrap();
    let hide = output
        .find("\x1b[?25l")
        .expect("repaint must hide the cursor first");
    let show = output
        .rfind("\x1b[?25h")
        .expect("repaint must show the cursor at the end");
    let first_paint = output
        .find("\x1b[?7l")
        .expect("repaint positions visual rows without autowrap");
    assert!(hide < first_paint, "cursor must be hidden before painting");
    assert!(show > hide);
    // Show 之后不再有任何绘制输出，光标不会再被移动
    assert!(!output[show + "\x1b[?25h".len()..].contains("\x1b[2K"));
}

/// 验证内容未变时跳过重绘，但仍把光标收回输入位置。
///
/// composer 每 32ms 刷新一次，逐行清除再打印在 Windows Terminal 下
/// 表现为底部闪烁；内容一致时必须不重绘内容。同时 transcript/动效
/// 绘制会把终端光标移走，必须恢复光标位置与显示，光标才始终留在输入框。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn skips_repaint_when_nothing_changed() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, "hello".to_string(), 5, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 40, rows: 12 }, frame.height(40), 8);

    let mut first = Vec::new();
    let (cursor_row, signature) = frame.draw_lines(&mut first, &viewport, None).unwrap();
    assert!(!first.is_empty(), "首次绘制必须实际输出");

    let mut second = Vec::new();
    let (second_row, _) = frame
        .draw_lines(&mut second, &viewport, Some(&signature))
        .unwrap();

    let output = String::from_utf8(second).unwrap();
    assert!(
        !output.contains("\x1b[2K"),
        "内容未变时不应重绘任何行: {output:?}"
    );
    assert!(
        output.contains("\x1b[?25h"),
        "内容未变时也要恢复光标显示: {output:?}"
    );
    assert_eq!(second_row, cursor_row);
}

/// 验证输入变化后仍会重绘。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn repaints_after_the_input_changes() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let first_frame =
        ComposerFrame::new(chrome.clone(), "hello".to_string(), 5, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(
        TerminalSize { cols: 40, rows: 12 },
        first_frame.height(40),
        8,
    );
    let mut sink = Vec::new();
    let (_, signature) = first_frame.draw_lines(&mut sink, &viewport, None).unwrap();

    let changed = ComposerFrame::new(chrome, "hello world".to_string(), 11, false, Vec::new(), 0);
    let mut output = Vec::new();
    changed
        .draw_lines(&mut output, &viewport, Some(&signature))
        .unwrap();

    assert!(!output.is_empty(), "输入变化后必须重绘");
}

/// 验证输入 `!` 时展示 shell 提示与幽灵说明，并隐藏常规底栏。
#[test]
fn bang_prefix_shows_shell_hint_instead_of_footer() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt-test".to_string(),
        thinking: "auto".to_string(),
        directory: "/tmp".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, "!".to_string(), 1, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
    let mut output = Vec::new();
    frame.draw_lines(&mut output, &viewport, None).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains('!'));
    assert!(
        output.contains("Run a command") || output.contains("运行命令"),
        "missing shell hint: {output}"
    );
    assert!(output.contains("gpt-test"));
    assert!(!output.contains("120k"));
}

/// 验证 slash 命令面板隐藏常规状态栏并展示命令说明。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn slash_panel_keeps_input_frame_visible_above_command_descriptions() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, "/".to_string(), 1, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
    let mut output = Vec::new();

    frame.draw_lines(&mut output, &viewport, None).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains("/help") && output.contains("/rename"),
        "slash panel should list visible commands: {output}"
    );
    // slash 展开时输入行仍在，但不画状态分隔线
    assert!(output.contains("/"));
    assert!(!output.contains("120k"));
}

/// 验证沉底面板行渲染在输入框顶线上方并计入高度。
#[test]
fn panel_lines_render_above_chrome_and_extend_height() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let mut frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
    let base_height = frame.height(72);
    frame.set_panel_lines(vec![
        "\x1b[2m• 1/3\x1b[0m".to_string(),
        "\x1b[1m\x1b[36m▶\x1b[0m \x1b[1m\x1b[36mcurrent\x1b[0m".to_string(),
    ]);
    assert_eq!(frame.height(72), base_height + 2);

    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
    let mut output = Vec::new();
    frame.draw_lines(&mut output, &viewport, None).unwrap();
    let output = String::from_utf8(output).unwrap();
    let panel_at = output.find("• 1/3").unwrap();
    // 极简输入行无彩条；面板须出现在输入提示之前
    // 占位提示逐轮轮换，测试取当前值而不是硬编码首条
    let tip = super::super::placeholder_tips::current_tip();
    let input_at = output.find(tip).expect("input placeholder");
    assert!(panel_at < input_at, "面板行必须渲染在输入区之前");
}

/// 验证空输入框显示灰色轮询提示。
#[test]
fn empty_composer_shows_placeholder() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
    let mut output = Vec::new();

    frame.draw_lines(&mut output, &viewport, None).unwrap();

    let output = String::from_utf8(output).unwrap();
    // 同一轮内提示保持静止，不随轮询变化
    let tip = super::super::placeholder_tips::current_tip();
    assert!(!tip.is_empty());
    assert!(output.contains(tip));
    assert!(output.contains("\x1b[2m"));
}

/// 验证悬浮 composer 绘制后清除其下方残留内容。
#[test]
fn floating_composer_clears_stale_rows_below() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 4);
    let mut output = Vec::new();

    frame.draw_lines(&mut output, &viewport, None).unwrap();

    let output = String::from_utf8(output).unwrap();
    // 上空白 1 + 内上边距 1 + 输入 1 + 内下边距 1 + 状态 1 = 5；
    // 顶部行 4（0 起）时，末行后为行 9 → 1 起第 10 行
    assert!(
        output.contains("\x1b[10;1H\x1b[J"),
        "expected clear below floating composer, got {output:?}"
    );
}

/// 验证贴底 composer 不发出越界清除，footer 行保持完整。
#[test]
fn bottom_pinned_composer_keeps_footer_row() {
    let chrome = ReplChrome {
        mode: AgentMode::Yolo,
        context_ratio: 0.0,
        context_window_tokens: 120_000,
        model: "gpt".to_string(),
        thinking: "auto".to_string(),
        directory: "/workspace".to_string(),
        cache_hit_ratio: None,
        activity: None,
        role_badge: None,
    };
    let frame = ComposerFrame::new(chrome, String::new(), 0, false, Vec::new(), 0);
    let mut viewport = InlineViewport::new();
    // 历史充满屏幕：composer 固定在底部，末行即屏幕最后一行
    viewport.update(TerminalSize { cols: 72, rows: 24 }, frame.height(72), 60);
    let mut output = Vec::new();

    frame.draw_lines(&mut output, &viewport, None).unwrap();

    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("\x1b[J"), "贴底时不能清除 footer 行");
    assert!(output.contains("gpt"));
}

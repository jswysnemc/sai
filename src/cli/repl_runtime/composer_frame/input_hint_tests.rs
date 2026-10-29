use super::*;
use crate::agent::AgentMode;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_runtime::composer_frame::ComposerFrame;
use crate::config::TuiMode;
use crate::render::tui_mode;

/// 【终端】【输入测试】构造空输入状态。
/// 参数: 无；返回: 默认空输入框。
fn frame() -> ComposerFrame {
    ComposerFrame::new(
        ReplChrome {
            mode: AgentMode::Yolo,
            context_ratio: 0.0,
            context_window_tokens: 120_000,
            model: "fixture".into(),
            thinking: "auto".into(),
            directory: "/workspace".into(),
            branch: None,
            cache_hit_ratio: None,
            status_plugin: None,
        },
        String::new(),
        0,
        false,
        Vec::new(),
        0,
    )
}

/// 【终端】【输入语言测试】全屏界面空闲与工作提示不随全局中文设置混用中文。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_input_hints_stay_english_under_chinese_locale() {
    crate::i18n::with_locale(crate::i18n::Locale::Zh, || {
        assert_eq!(placeholder(false, 80), "Enter a message or type /help");
        assert_eq!(placeholder(true, 80), "Add a follow-up to the queue");
    });
}

/// 【终端】【输入测试】工作与空闲提示符合当前动作，缩窄时优先保留输入引导。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_input_hints_match_state_and_width() {
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        assert_eq!(placeholder(false, 80), "Enter a message or type /help");
        assert_eq!(placeholder(true, 80), "Add a follow-up to the queue");
        assert_eq!(placeholder(false, 20), "Enter a message");
        assert_eq!(placeholder(true, 20), "Queue a follow-up");
        assert_eq!(placeholder(false, 10), "Message");
        for width in 1..7 {
            assert_eq!(placeholder(false, width), "");
            assert_eq!(placeholder(true, width), "");
        }
    });
    crate::i18n::with_locale(crate::i18n::Locale::Zh, || {
        assert_eq!(placeholder(false, 8), "Message");
        assert_eq!(placeholder(true, 12), "Follow-up");
    });
}

/// 【终端】【输入测试】临时输入提示优先于简洁默认提示，内联界面继续使用轮换提示。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_input_hints_preserve_overrides_and_inline() {
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    crate::i18n::with_locale(crate::i18n::Locale::En, || {
        let mut frame = frame();
        for width in [1, 2, 3, 4, 5, 8, 10, 20, 30, 80] {
            assert_eq!(frame.layout(width).input_rows, 1, "terminal width={width}");
        }
        assert!(frame
            .layout(80)
            .styled_display_lines
            .join("\n")
            .contains("Enter a message"));
        frame.set_streaming(true);
        assert!(frame
            .layout(80)
            .styled_display_lines
            .join("\n")
            .contains("Add a follow-up"));
        frame.set_placeholder_override(Some("SSH input required".into()));
        assert!(frame
            .layout(80)
            .styled_display_lines
            .join("\n")
            .contains("SSH input required"));
        frame.set_placeholder_override(None);
        tui_mode::set_override(Some(TuiMode::Inline));
        let inline = frame.layout(80).styled_display_lines.join("\n");
        assert!(!inline.is_empty());
        assert!(!inline.contains("Enter a message or type /help"));
        assert!(!inline.contains("Add a follow-up to the queue"));
    });
    tui_mode::set_override(None);
}

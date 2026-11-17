use super::*;
use crate::config::TuiMode;
use crate::render::transcript::cell::{HistoryCell, TranscriptMode};
use crate::render::transcript::render_cache::RenderCache;
use crate::render::transcript::user_echo_cell::UserEchoCell;
use crate::render::tui_mode;

/// 【终端】【简洁测试】紧凑欢迎信息在不同窗口宽度保留模型与路径。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_welcome_is_compact_and_responsive() {
    let cell = WelcomeCell {
        version: "0.2.10".to_string(),
        model: "deepseek-v4".to_string(),
        directory: "~/workspace/sai".to_string(),
        permissions: "audit".to_string(),
    };
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    for width in [1, 2, 3, 4, 20, 40, 80, 120] {
        let lines = welcome_lines(&cell, width);
        assert!(lines
            .iter()
            .all(|line| unicode_width::UnicodeWidthStr::width(
                ansi.replace_all(line.as_str(), "").as_ref()
            ) <= width));
        if width >= 40 {
            assert_eq!(lines.len(), 3);
            let text = lines
                .iter()
                .map(|line| line.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("deepseek-v4"));
            assert!(!text.contains("Fullscreen"));
            assert!(!text.contains("DeepSeek harness"));
            assert!(text.contains("~/workspace/sai"));
            assert!(text.contains('╭'));
        }
    }
}

/// 【终端】【简洁测试】相同宽度下切换样式必须使历史渲染缓存失效。
/// 参数: 无；返回: 无。
#[test]
fn view_switch_invalidates_transcript_cache() {
    let cell = HistoryCell::Welcome(WelcomeCell {
        version: "0.2.10".into(),
        model: "fixture".into(),
        directory: "~/workspace/sai".into(),
        permissions: "yolo".into(),
    });
    let options = super::super::test_support::options();
    let mut cache = RenderCache::default();
    tui_mode::set_override(Some(TuiMode::Inline));
    let inline = cache.lines_for(0, &cell, 40, &options, 0);
    tui_mode::set_override(Some(TuiMode::Fullscreen));
    let count = cache.count_for(0, &cell, 40, &options, 0);
    let fullscreen = cache.lines_for(0, &cell, 40, &options, 0);
    assert_ne!(inline, fullscreen);
    assert_eq!(count, fullscreen.len());
    assert!(fullscreen
        .iter()
        .any(|line| line.as_str().contains("fixture")));
    tui_mode::set_override(Some(TuiMode::Inline));
    assert_eq!(cache.lines_for(0, &cell, 40, &options, 0), inline);
    tui_mode::set_override(None);
}

/// 【终端】【用户消息层级测试】全屏模式保持纯文本引导列不变，并以粗体和醒目色阶同助手正文首行拉开区分。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[test]
fn fullscreen_user_message_keeps_plain_layout_and_distinguishes_from_assistant_body() {
    let assistant_first = assistant_lines("助手首行回答", 80).remove(0);
    assert!(assistant_first.as_str().starts_with("\x1b[2m●\x1b[0m "));

    for mode in [
        TranscriptMode::Yolo,
        TranscriptMode::Plan,
        TranscriptMode::Automatic,
    ] {
        let cell = UserEchoCell::new(mode, "用户消息\n第二行 source.rs".into());
        tui_mode::set_override(Some(TuiMode::Inline));
        let inline = super::super::user_echo_cell::render(&cell);
        tui_mode::set_override(Some(TuiMode::Fullscreen));
        let fullscreen = super::super::user_echo_cell::render(&cell);
        assert_eq!(
            crate::render::activity_animation::strip_ansi_for_test(&fullscreen),
            crate::render::activity_animation::strip_ansi_for_test(&inline)
        );
        assert!(fullscreen.contains('●'));
        assert!(!fullscreen.contains('›'));
        assert!(fullscreen.contains("\x1b[1m用户消息"));
        assert!(!fullscreen.contains("\x1b[2m●\x1b[0m "));
    }
    tui_mode::set_override(None);
}

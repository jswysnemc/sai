use super::*;
use crate::config::TuiStyle;
use crate::render::transcript::cell::{HistoryCell, TranscriptMode};
use crate::render::transcript::render_cache::RenderCache;
use crate::render::transcript::user_echo_cell::UserEchoCell;
use crate::render::tui_style;

/// 【终端】【简洁测试】紧凑欢迎信息在不同窗口宽度保留模型与路径。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_welcome_is_compact_and_responsive() {
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
            assert!(text.contains("DeepSeek style"));
            assert!(!text.contains("DeepSeek harness"));
            assert!(text.contains("~/workspace/sai"));
            assert!(!text.contains('╭'));
        }
    }
}

/// 【终端】【简洁测试】相同宽度下切换样式必须使历史渲染缓存失效。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_switch_invalidates_transcript_cache() {
    let cell = HistoryCell::Welcome(WelcomeCell {
        version: "0.2.10".into(),
        model: "fixture".into(),
        directory: "~/workspace/sai".into(),
        permissions: "yolo".into(),
    });
    let options = super::super::test_support::options();
    let mut cache = RenderCache::default();
    tui_style::set_override(Some(TuiStyle::Classic));
    let classic = cache.lines_for(0, &cell, 40, &options, 0);
    tui_style::set_override(Some(TuiStyle::Deepseek));
    let count = cache.count_for(0, &cell, 40, &options, 0);
    let deepseek = cache.lines_for(0, &cell, 40, &options, 0);
    assert_ne!(classic, deepseek);
    assert_eq!(count, deepseek.len());
    assert!(deepseek
        .iter()
        .any(|line| line.as_str().contains("DeepSeek style")));
    tui_style::set_override(Some(TuiStyle::Classic));
    assert_eq!(cache.lines_for(0, &cell, 40, &options, 0), classic);
    tui_style::set_override(None);
}

/// 【终端】【用户消息回滚测试】两种样式使用相同模式标记，不修改中文或多行内容。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_user_message_matches_classic_for_every_mode() {
    for mode in [
        TranscriptMode::Yolo,
        TranscriptMode::Plan,
        TranscriptMode::Automatic,
    ] {
        let cell = UserEchoCell::new(mode, "用户消息\n第二行 source.rs".into());
        tui_style::set_override(Some(TuiStyle::Classic));
        let classic = super::super::user_echo_cell::render(&cell);
        tui_style::set_override(Some(TuiStyle::Deepseek));
        let deepseek = super::super::user_echo_cell::render(&cell);
        assert_eq!(deepseek, classic);
        assert!(deepseek.contains('●'));
        assert!(!deepseek.contains('›'));
        assert!(deepseek.contains("用户消息"));
    }
    tui_style::set_override(None);
}

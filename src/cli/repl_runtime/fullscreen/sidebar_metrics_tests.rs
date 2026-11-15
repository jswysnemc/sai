use super::sidebar_metrics::*;

/// 【终端指标测试】【标题避让】参数：无；返回：无，验证工作区和分支不占用右上角开关范围
#[test]
fn heading_reserves_columns_for_sidebar_toggle() {
    let mut chrome = super::runtime_tests::chrome();
    chrome.directory = "~/workspace/sai".into();
    chrome.branch = Some("main".into());
    let metrics = SidebarMetrics::new(Some(&chrome), &Default::default());
    let header = metrics.heading(37, 30);
    assert!(crate::render::ansi_clip::strip_ansi(&header[0]).ends_with("main"));
    assert!(crate::cli::repl_text::visible_width(&header[0]) <= 33);
}

/// 【终端】【指标降级】未知数据不伪造零值，短屏不占用操作入口
#[test]
fn unknown_metrics_and_short_screens_are_supported() {
    let metrics = SidebarMetrics::default();
    assert!(metrics.rows(30, 15).is_empty());
    let rows = metrics.rows(30, 30);
    assert_eq!(rows.len(), 2);
    assert!(crate::render::ansi_clip::strip_ansi(&rows[1]).contains("Cache --"));
    for width in [1, 10, 25] {
        for line in metrics.rows(width, 30) {
            assert!(crate::render::ansi_clip::ansi_visible_width(&line) <= width);
        }
    }
}

/// 【终端指标】【参考排版】无参数；返回无，确保百万窗口和小比例保持一位精度
#[test]
fn context_readout_matches_compact_reference() {
    assert_eq!(
        super::sidebar_metrics_view::context(Some((12_500, 1_000_000))),
        "12.5k / 1M · 1.3%"
    );
    assert_eq!(super::sidebar_metrics_view::tokens(100_000), "100k");
    assert!(super::sidebar_metrics_view::context(Some((12, 0))).contains("--"));
}

/// 【终端指标】【分段边界】无参数；返回无，已用量非零保留标记，过量时不超出轨道
#[test]
fn segmented_bar_handles_empty_full_and_overflow() {
    use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};
    for width in [0, 1, 10, 28, 40] {
        for context in [
            None,
            Some((0, 100)),
            Some((1, 100)),
            Some((100, 100)),
            Some((200, 100)),
            Some((2, 0)),
        ] {
            let bar = super::sidebar_metrics_view::bar(context, width);
            assert!(ansi_visible_width(&bar) <= width);
            let text = strip_ansi(&bar);
            if width > 0 && context == Some((1, 100)) {
                assert!(text.contains('▰'));
            }
            if context == None || context == Some((2, 0)) {
                assert!(!text.contains('▰'));
            }
        }
    }
}

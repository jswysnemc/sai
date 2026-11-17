use super::sidebar_metrics::*;

/// 【终端指标测试】【双行标题】验证目录与分支分两行独立展示且不超出侧栏净宽。
#[test]
fn heading_splits_directory_and_branch_across_two_lines() {
    let mut chrome = super::runtime_tests::chrome();
    chrome.directory = "~/workspace/sai".into();
    chrome.branch = Some("main".into());
    let metrics = SidebarMetrics::new(Some(&chrome), &Default::default());
    let header = metrics.heading(35, 30);
    assert_eq!(header.len(), 2);
    let dir_line = crate::render::ansi_clip::strip_ansi(&header[0]);
    let branch_line = crate::render::ansi_clip::strip_ansi(&header[1]);
    assert!(dir_line.contains("~/workspace/sai"), "{dir_line}");
    assert!(branch_line.contains("main"), "{branch_line}");
    assert!(crate::cli::repl_text::visible_width(&header[0]) <= 35);
    assert!(crate::cli::repl_text::visible_width(&header[1]) <= 35);
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

/// 【终端指标】【简约进度条边界】验证非零占用至少点亮一格横线，零占用与过量均不越界且无杂乱符号。
#[test]
fn minimalist_bar_handles_empty_full_and_overflow() {
    use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};
    for width in [0, 1, 10, 24, 40] {
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
            assert!(
                text.chars().all(|ch| matches!(ch, '━' | '─')),
                "简约上下文条仅包含同轴横线字符: {text}"
            );
            if width > 0 && context == Some((1, 100)) {
                assert!(text.contains('━'), "微小非零占用应至少点亮一格: {text}");
            }
            if context.is_none() || context == Some((0, 100)) || context == Some((2, 0)) {
                assert!(!text.contains('━'), "零占用或未知窗口不应点亮实心段: {text}");
            }
        }
    }
}

/// 【终端指标】【阈值与扩展】验证零负载不加粗抢戏、72%/90% 警告色阶及宽栏简约轨道扩展
#[test]
fn wide_sidebar_expands_minimalist_gauge_and_uses_warning_thresholds() {
    let mut idle_chrome = super::runtime_tests::chrome();
    idle_chrome.context_window_tokens = 960_000;
    idle_chrome.context_ratio = 0.0;
    let idle_rows = SidebarMetrics::new(Some(&idle_chrome), &Default::default()).rows(38, 34);
    assert!(!idle_rows[0].contains("\x1b[1m"), "零负载不应加粗百分比");

    let mut chrome = super::runtime_tests::chrome();
    chrome.context_window_tokens = 400_000;
    chrome.context_ratio = 0.78;
    let metrics = SidebarMetrics::new(Some(&chrome), &Default::default());
    let rows = metrics.rows(38, 34);
    assert!(rows[0].contains("\x1b[1m"), "高水位应强调百分比");
    let plain = crate::render::ansi_clip::strip_ansi(&rows[0]);
    let gauge_cells = plain
        .chars()
        .filter(|ch| matches!(ch, '━' | '─'))
        .count();
    assert!(gauge_cells > 10, "宽栏应展开简约横线轨道: {plain}");

    let palette = crate::render::panel_palette::PanelPalette::current();
    let warn_bar = super::sidebar_metrics_view::bar(Some((78, 100)), 20);
    assert!(warn_bar.contains(&crate::render::panel_palette::fg(palette.warning)));
    let danger_bar = super::sidebar_metrics_view::bar(Some((92, 100)), 20);
    assert!(danger_bar.contains(&crate::render::panel_palette::fg(palette.removed)));
}

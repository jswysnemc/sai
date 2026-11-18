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
    assert_eq!(rows.len(), 4);
    assert!(crate::render::ansi_clip::strip_ansi(&rows[3]).contains("C--"));
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

/// 【终端指标】【分段边界】零占用、过量、未知缓存与窄宽度均保持轨道容量边界。
/// 参数: 无；返回: 无。
#[test]
fn segmented_bar_handles_empty_full_and_overflow() {
    use super::sidebar_context_bar::ContextBar;
    use crate::render::ansi_clip::{ansi_visible_width, strip_ansi};
    for width in [0, 1, 2, 10, 24, 40, 80] {
        for used in [0, 1, 45, 80, 100, 200] {
            for cached in [None, Some(0), Some(40), Some(200)] {
                let bar = ContextBar {
                    used,
                    window: 100,
                    trigger: 80,
                    cached,
                };
                let track = bar.track(width);
                assert_eq!(ansi_visible_width(&track), width);
                let text = strip_ansi(&track);
                assert_eq!(text.matches('┆').count(), usize::from(width > 0));
                if width > 1 && used > 0 {
                    assert!(text.contains('▰'), "{text}");
                }
                if used == 0 {
                    assert!(!text.contains('▰'), "{text}");
                }
                assert!(ansi_visible_width(&bar.heading(width)) <= width);
                assert!(ansi_visible_width(&bar.legend(width)) <= width);
            }
        }
    }
}

/// 【终端指标】【真实触发点】使用会话比例与预留策略计算余量，不采用固定演示阈值。
/// 参数: 无；返回: 无。
#[test]
fn context_bar_uses_session_compaction_policy() {
    use crate::render::ansi_clip::strip_ansi;
    let mut chrome = super::runtime_tests::chrome();
    chrome.context_window_tokens = 400_000;
    chrome.context_ratio = 0.1;
    chrome.compaction_policy = crate::state::CompactionBudgetPolicy::from_context(0.8, 0)
        .with_mode(crate::config::CompactionMode::Ratio);
    let rows = SidebarMetrics::new(Some(&chrome), &Default::default()).rows(48, 34);
    assert!(
        strip_ansi(&rows[0]).contains("280k"),
        "{}",
        strip_ansi(&rows[0])
    );
    assert!(strip_ansi(&rows[2]).contains("80k"));
    assert_eq!(
        strip_ansi(&rows[1]).chars().position(|ch| ch == '┆'),
        Some(38)
    );
    chrome.compaction_policy = crate::state::CompactionBudgetPolicy::from_context(0.8, 50_000)
        .with_mode(crate::config::CompactionMode::Reserve);
    let rows = SidebarMetrics::new(Some(&chrome), &Default::default()).rows(48, 34);
    assert!(strip_ansi(&rows[0]).contains("310k"));
    assert!(strip_ansi(&rows[2]).contains("50k"));
}

/// 【终端指标】【缓存口径】分段使用最新请求缓存量，避免整轮累计比例误导当前成分。
/// 参数: 无；返回: 无。
#[test]
fn context_segments_use_latest_request_cache_tokens() {
    let chrome = super::runtime_tests::chrome();
    let mut live = crate::cli::repl_runtime::live_usage::LiveTurnUsage::default();
    for (prompt, cache_read) in [(10_000, 10_000), (20_000, 5_000)] {
        live.record(&crate::llm::Usage {
            prompt_tokens: prompt,
            cache_read_tokens: cache_read,
            ..Default::default()
        });
    }
    let metrics = SidebarMetrics::new(Some(&chrome), &live);
    let rows = metrics.rows(48, 34);
    assert!(crate::render::ansi_clip::strip_ansi(&rows[0]).contains("20k"));
    assert!(crate::render::ansi_clip::strip_ansi(&rows[3]).contains("50%"));
    let legend = crate::render::ansi_clip::strip_ansi(&rows[2]);
    assert!(legend.contains("5k") && legend.contains("15k"), "{legend}");
}

/// 【终端指标】【全宽轨道】正常高度使用整行轨道，矮屏双行降级且零负载不加粗。
/// 参数: 无；返回: 无。
#[test]
fn metrics_adapt_to_sidebar_width_and_height() {
    let mut chrome = super::runtime_tests::chrome();
    chrome.context_window_tokens = 400_000;
    chrome.context_ratio = 0.0;
    let metrics = SidebarMetrics::new(Some(&chrome), &Default::default());
    for width in [1, 10, 25, 38, 64] {
        for height in [8, 16, 21, 22, 34] {
            let rows = metrics.rows(width, height);
            assert_eq!(rows.len(), super::sidebar_metrics::height(height));
            for line in &rows {
                assert!(crate::render::ansi_clip::ansi_visible_width(line) <= width);
                assert!(!line.contains("\x1b[1m"));
            }
            if height >= 22 {
                assert_eq!(
                    crate::render::ansi_clip::ansi_visible_width(&rows[1]),
                    width
                );
            }
        }
    }
}

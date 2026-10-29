use super::*;

/// 【上下文】【参考数据】构造与参考图同量级的上下文，不读取用户会话。
/// 参数: 无；返回: 浮层测试数据。
fn fixture() -> ContextOverview {
    ContextOverview {
        used: 33_000,
        window: 1_000_000,
        compact_trigger: 850_000,
        categories: vec![
            CategoryUsage {
                label: "系统提示词".into(),
                tokens: 18_200,
                color: (255, 214, 165),
            },
            CategoryUsage {
                label: "工具定义".into(),
                tokens: 12_300,
                color: (205, 165, 168),
            },
            CategoryUsage {
                label: "你的消息".into(),
                tokens: 118,
                color: (132, 213, 172),
            },
            CategoryUsage {
                label: "模型回复".into(),
                tokens: 77,
                color: (211, 131, 135),
            },
            CategoryUsage {
                label: "工具调用".into(),
                tokens: 924,
                color: (255, 180, 107),
            },
            CategoryUsage {
                label: "工具结果".into(),
                tokens: 1_381,
                color: (207, 170, 133),
            },
        ],
        usage: Some(crate::llm::Usage {
            prompt_tokens: 32_952,
            cache_read_tokens: 32_600,
            cache_write_tokens: 0,
            completion_tokens: 267,
            total_tokens: 33_219,
        }),
        usage_is_turn: true,
        external_engine: None,
    }
}

/// 【上下文】【预算边界】空闲区和预留区不重叠，接近满窗时预留量随剩余空间减少。
/// 参数: 无；返回: 无。
#[test]
fn context_budget_regions_do_not_overlap() {
    let mut view = fixture();
    assert_eq!(view.free_and_reserved(), (817_000, 150_000));
    view.used = 920_000;
    assert_eq!(view.free_and_reserved(), (0, 80_000));
    view.used = 1_100_000;
    assert_eq!(view.free_and_reserved(), (0, 0));
}

/// 【上下文】【响应式回归】宽屏图例位于图表右侧，窄屏安全换行，不生成超宽行。
/// 参数: 无；返回: 无。
#[test]
fn context_panel_fits_terminal_widths() {
    crate::i18n::with_locale(crate::i18n::Locale::Zh, || {
        for width in [12, 24, 40, 72, 100, 120] {
            let lines = render(&fixture(), width, true);
            assert!(
                lines
                    .iter()
                    .all(|line| crate::render::ansi_clip::ansi_visible_width(line) <= width),
                "width={width}: {lines:?}"
            );
        }
        let plain = crate::render::ansi_clip::strip_ansi(&render(&fixture(), 110, true).join("\n"));
        for expected in [
            "33.0K / 1.0M",
            "817.0K",
            "150.0K",
            "85%",
            "输入 352",
            "缓存命中 32.6K (99%)",
            "输出 267",
        ] {
            assert!(plain.contains(expected), "{plain}");
        }
        assert!(!plain.contains("MCP 工具") && !plain.contains('□'));
        let narrow =
            crate::render::ansi_clip::strip_ansi(&render(&fixture(), 60, false).join("\n"));
        assert!(narrow.contains("输出 267"), "{narrow}");
    });
}

/// 【上下文】【空用量回归】无用量记录时给出明确提示，不把未上报等同零开销。
/// 参数: 无；返回: 无。
#[test]
fn context_panel_handles_empty_window() {
    let view = ContextOverview::default();
    let text = render(&view, 80, false).join("\n");
    assert!(!text.contains("NaN") && !text.contains("inf"));
    assert!(text.contains("No usage reported") || text.contains("尚无用量数据"));
}

/// 【上下文】【小分类可见】单元足够时每个非零分类都有色块，图表单元总数不变。
/// 参数: 无；返回: 无。
#[test]
fn context_chart_keeps_small_categories_visible() {
    let view = fixture();
    let chart = chart::render(&view, 20, 5, true).join("\n");
    for category in &view.categories {
        let (r, g, b) = category.color;
        assert!(
            chart.contains(&format!("38;2;{r};{g};{b}m"))
                || chart.contains(&format!("48;2;{r};{g};{b}m")),
            "{} missing: {chart:?}",
            category.label
        );
    }
    let plain = chart::render(&view, 20, 5, false);
    assert_eq!(plain.len(), 5);
    assert!(plain
        .iter()
        .all(|line| unicode_width::UnicodeWidthStr::width(line.as_str()) == 39));
}

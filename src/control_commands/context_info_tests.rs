use super::*;

fn view(used: usize, window: usize, categories: Vec<CategoryUsage>) -> ContextView {
    ContextView {
        model_name: "Opus".into(),
        model_id: "claude-opus".into(),
        used,
        window,
        categories,
        mcp_tools: vec![
            ("mcp_memory_search".into(), 12),
            ("mcp_memory_index".into(), 0),
        ],
        compaction: None,
        compact_ratio: 0.9,
        compact_reserve: 50_000,
        compact_trigger: 950_000,
        session_override: false,
    }
}

fn skills(tokens: usize) -> CategoryUsage {
    CategoryUsage {
        label: "Skills".into(),
        tokens,
        color: "\x1b[33m",
    }
}

/// 占用不足 1% 时仍点亮一格，空白占满其余。
#[test]
fn tiny_usage_lights_one_cell() {
    let categories = vec![skills(4_200)];
    let cells = allocate_cells(&categories, 4_200, 1_000_000);
    assert_eq!(cells, vec![1]);
    let rendered = render_context_view(&view(4_200, 1_000_000, categories), false);
    let filled = rendered.matches(FILLED).count();
    // 网格 1 格 + 图例 1 格
    assert_eq!(filled, 2, "{rendered}");
    assert!(rendered.contains("4.2k/1M"));
    assert!(rendered.contains("(0%)"));
    assert!(rendered.contains("Skills: 4.2k"));
    assert!(rendered.contains("(0.4%)"));
    assert!(rendered.contains("995.8k"));
    assert!(rendered.contains("99.6%"));
    assert!(rendered.contains("50k"));
    assert!(rendered.contains("950k"));
}

/// MCP 列表用树形线，末项为 └──。
#[test]
fn mcp_tools_render_as_a_tree() {
    let rendered = render_context_view(&view(0, 128_000, Vec::new()), false);
    assert!(rendered.contains("├── mcp_memory_search:"));
    assert!(rendered.contains("└── mcp_memory_index:"));
    assert!(rendered.contains("└ "));
}

/// 全空窗口只画空白格，不出现实心方块。
#[test]
fn empty_window_has_no_filled_cells_in_the_grid() {
    let rendered = render_context_view(&view(0, 100, Vec::new()), false);
    let grid = rendered
        .lines()
        .filter(|line| line.contains(EMPTY) && line.contains("  "))
        .take(10)
        .collect::<Vec<_>>();
    assert_eq!(grid.len(), 10);
    let grid_filled = grid
        .iter()
        .map(|line| line.matches(FILLED).count())
        .sum::<usize>();
    assert_eq!(grid_filled, 0);
}

use crate::agent::AgentMode;
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::token_estimate;
use anyhow::Result;

/// 10×10 方格，每格约占上下文窗口的 1%。
const GRID_CELLS: usize = 100;
const GRID_COLS: usize = 10;
const FILLED: char = '■';
const EMPTY: char = '□';

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const ITALIC: &str = "\x1b[3m";

/// 按当前 REPL 模式组装着色的上下文用量视图。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式（影响可见工具与 MCP 估算）
///
/// 返回:
/// - 带 ANSI 的多行文本
#[allow(dead_code)]
pub fn context_info_for_mode(paths: &SaiPaths, mode: AgentMode) -> Result<String> {
    context_info_for(paths, mode, true, None)
}

/// 组装无 ANSI 的上下文用量视图，供网关渠道回复。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 多行纯文本
#[allow(dead_code)]
pub fn context_info_plain(paths: &SaiPaths) -> Result<String> {
    context_info_for(paths, AgentMode::Yolo, false, None)
}

/// 查看上下文用量，并可改写本会话压缩策略。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `mode`: 当前 Agent 模式
/// - `update`: 对本会话策略的改写
///
/// 返回:
/// - 带 ANSI 的多行文本
pub fn context_info_for_mode_with_update(
    paths: &SaiPaths,
    mode: AgentMode,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<String> {
    context_info_for(paths, mode, true, update)
}

/// 网关入口：查看或改写本会话压缩策略。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `update`: 对本会话策略的改写
///
/// 返回:
/// - 多行纯文本
pub fn context_info_plain_with_update(
    paths: &SaiPaths,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<String> {
    context_info_for(paths, AgentMode::Yolo, false, update)
}

/// 读取会话状态并渲染上下文用量。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `mode`: Agent 模式
/// - `styled`: 是否输出 ANSI 颜色
///
/// 返回:
/// - 渲染文本
fn context_info_for(
    paths: &SaiPaths,
    mode: AgentMode,
    styled: bool,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    let state = crate::state::StateStore::new(paths)?;
    apply_context_policy_update(&state, &config, update)?;
    let context_limit = config.active_context_window_tokens().unwrap_or(128_000);
    let snapshot = state.session_snapshot(context_limit)?;
    let provider = config.provider(None).ok();
    let model_id = provider
        .map(|item| item.default_model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| "-".to_string());
    let model_name = provider
        .map(|item| {
            let name = item.display_name.trim();
            if name.is_empty() {
                item.id.clone()
            } else {
                name.to_string()
            }
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| model_id.clone());

    if config.agent.engine.is_external() {
        return Ok(render_external_engine(
            config.agent.engine.display_label(),
            styled,
        ));
    }

    let workspace = crate::runtime_cwd::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let breakdown = crate::web::services::context_breakdown::estimate_context_breakdown(
        &config, paths, &state, &workspace, mode,
    )
    .ok();
    let categories = category_usages(breakdown.as_ref(), snapshot.context_prompt_tokens);
    let mcp_tools = mcp_tool_usages(&config, paths, mode);
    let compaction = snapshot
        .compaction
        .as_ref()
        .map(|item| item.compacted_turns);
    let resolved = state.resolve_compaction_policy(&config.context)?;
    let compact_ratio = resolved.policy.ratio;
    let compact_reserve = resolved.policy.reserve_tokens;
    let compact_trigger = crate::state::CompactionBudgetPolicy::from_context(
        compact_ratio,
        compact_reserve,
    )
    .trigger_chars(snapshot.context_window_tokens.max(1));

    Ok(render_context_view(
        &ContextView {
            model_name,
            model_id,
            used: snapshot.context_prompt_tokens,
            window: snapshot.context_window_tokens.max(1),
            categories,
            mcp_tools,
            compaction,
            compact_ratio,
            compact_reserve,
            compact_trigger,
            session_override: resolved.session_override,
        },
        styled,
    ))
}

/// 渲染外部内核的简短说明：用量由对方自行管理。
///
/// 参数:
/// - `engine`: 内核展示名
/// - `styled`: 是否着色
///
/// 返回:
/// - 说明文本
fn render_external_engine(engine: &str, styled: bool) -> String {
    let title = paint(
        styled,
        DIM,
        &format!("└ {}", t("Context Usage", "上下文用量")),
    );
    let body = t("managed by the external engine", "由外部内核自行管理");
    format!("{title}\n  {engine} · {body}")
}

/// /context 视图的纯数据，便于单测渲染而不碰文件系统。
struct ContextView {
    model_name: String,
    model_id: String,
    used: usize,
    window: usize,
    categories: Vec<CategoryUsage>,
    mcp_tools: Vec<(String, usize)>,
    compaction: Option<usize>,
    compact_ratio: f32,
    compact_reserve: usize,
    compact_trigger: usize,
    session_override: bool,
}

struct CategoryUsage {
    label: String,
    tokens: usize,
    color: &'static str,
}

/// 把会话分项映射为带颜色的图例行。
///
/// 参数:
/// - `breakdown`: 可选的 token 分项
/// - `fallback_used`: 无分项时的总量
///
/// 返回:
/// - 非零分项；全空时退回单一“已使用”项
fn category_usages(
    breakdown: Option<&crate::web::services::context_breakdown::ContextUsageBreakdown>,
    fallback_used: usize,
) -> Vec<CategoryUsage> {
    let Some(raw) = breakdown else {
        return if fallback_used == 0 {
            Vec::new()
        } else {
            vec![CategoryUsage {
                label: t("Used", "已使用").to_string(),
                tokens: fallback_used,
                color: "\x1b[36m",
            }]
        };
    };
    let items = [
        (
            t("System prompt", "系统提示词"),
            raw.system_prompt_tokens,
            "\x1b[36m",
        ),
        (
            t("Tools & subagents", "工具及子智能体"),
            raw.tools_and_agents_tokens,
            "\x1b[34m",
        ),
        (
            t("Conversation", "对话消息"),
            raw.conversation_tokens,
            "\x1b[35m",
        ),
        (
            t("MCP tools", "MCP 工具"),
            raw.connectors_and_mcp_tokens,
            "\x1b[32m",
        ),
        (t("Skills", "技能"), raw.skills_tokens, "\x1b[33m"),
    ];
    let mut categories = items
        .into_iter()
        .filter(|(_, tokens, _)| *tokens > 0)
        .map(|(label, tokens, color)| CategoryUsage {
            label: label.to_string(),
            tokens,
            color,
        })
        .collect::<Vec<_>>();
    if categories.is_empty() && fallback_used > 0 {
        categories.push(CategoryUsage {
            label: t("Used", "已使用").to_string(),
            tokens: fallback_used,
            color: "\x1b[36m",
        });
    }
    categories
}

/// 估算当前可见 MCP 工具各自占用的 token。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `mode`: Agent 模式
///
/// 返回:
/// - `(工具名, token)`，按名称排序
fn mcp_tool_usages(config: &AppConfig, paths: &SaiPaths, mode: AgentMode) -> Vec<(String, usize)> {
    let Ok(registry) = crate::cli::build_tool_registry_with_cached_mcp(config, paths, mode) else {
        return Vec::new();
    };
    let mut tools = registry
        .definitions()
        .into_iter()
        .filter(|definition| is_mcp_tool_name(&definition.function.name))
        .map(|definition| {
            let serialized = serde_json::to_string(&definition).unwrap_or_else(|_| {
                format!(
                    "{}{}",
                    definition.function.name, definition.function.description
                )
            });
            (
                definition.function.name,
                token_estimate::estimate_tokens(&serialized),
            )
        })
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| left.0.cmp(&right.0));
    tools.truncate(24);
    tools
}

/// 判断是否为 MCP 工具（不含管理器本身）。
///
/// 参数:
/// - `name`: 工具名
///
/// 返回:
/// - 是否应出现在 MCP 列表
fn is_mcp_tool_name(name: &str) -> bool {
    name.starts_with("mcp_") && name != "mcp_manager"
}

/// 渲染 Claude Code 风格的上下文用量：左网格、右图例、下方 MCP 树。
///
/// 参数:
/// - `view`: 视图数据
/// - `styled`: 是否着色
///
/// 返回:
/// - 多行文本
fn render_context_view(view: &ContextView, styled: bool) -> String {
    let window = view.window.max(1);
    let used = view.used.min(window);
    let cells = allocate_cells(&view.categories, used, window);
    let grid = paint_grid(&view.categories, &cells, styled);
    let legend = legend_lines(view, used, window, styled);
    let mut lines = vec![paint(
        styled,
        DIM,
        &format!("└ {}", t("Context Usage", "上下文用量")),
    )];
    let row_count = GRID_CELLS / GRID_COLS;
    for row in 0..row_count {
        let grid_row = render_grid_row(&grid, row);
        let legend_row = legend.get(row).map(String::as_str).unwrap_or("");
        if legend_row.is_empty() {
            lines.push(format!("  {grid_row}"));
        } else {
            lines.push(format!("  {grid_row}  {legend_row}"));
        }
    }
    if legend.len() > row_count {
        for extra in &legend[row_count..] {
            lines.push(format!("                      {extra}"));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        "  {}",
        paint(
            styled,
            DIM,
            &format_auto_compact_line(view)
        )
    ));
    if let Some(turns) = view.compaction {
        lines.push(format!(
            "  {}",
            paint(
                styled,
                DIM,
                &format!(
                    "{} · {} {}",
                    t("Compaction", "压缩"),
                    turns,
                    t("turns compacted", "轮已压缩")
                )
            )
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "  {}",
        paint(styled, DIM, &t("MCP tools", "MCP 工具").to_string())
    ));
    if view.mcp_tools.is_empty() {
        lines.push(format!(
            "  {}",
            paint(
                styled,
                DIM,
                &format!("└── {}", t("none loaded", "尚未加载"))
            )
        ));
    } else {
        let last = view.mcp_tools.len() - 1;
        for (index, (name, tokens)) in view.mcp_tools.iter().enumerate() {
            let branch = if index == last {
                "└──"
            } else {
                "├──"
            };
            let count = paint(
                styled,
                DIM,
                &format!("{} {}", format_tokens(*tokens), t("tokens", "token")),
            );
            lines.push(format!("  {branch} {name}: {count}"));
        }
    }
    lines.join("\n")
}

/// 按窗口占比把 100 格分配给各分项，剩余为空白。
///
/// 占用不足一格但大于 0 时进位到 1 格，与参考实现一致。
///
/// 参数:
/// - `categories`: 非零分项
/// - `used`: 已用 token
/// - `window`: 窗口 token
///
/// 返回:
/// - 与分项等长的格子数
fn allocate_cells(categories: &[CategoryUsage], used: usize, window: usize) -> Vec<usize> {
    if categories.is_empty() || used == 0 || window == 0 {
        return vec![0; categories.len()];
    }
    let used_cells = ((used as f64 / window as f64) * GRID_CELLS as f64)
        .ceil()
        .clamp(1.0, GRID_CELLS as f64) as usize;
    let total = categories
        .iter()
        .map(|item| item.tokens)
        .sum::<usize>()
        .max(1);
    let mut cells = categories
        .iter()
        .map(|item| used_cells * item.tokens / total)
        .collect::<Vec<_>>();
    let assigned = cells.iter().sum::<usize>();
    let mut remainders = categories
        .iter()
        .enumerate()
        .map(|(index, item)| (index, (used_cells * item.tokens) % total))
        .collect::<Vec<_>>();
    remainders.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    let mut leftover = used_cells.saturating_sub(assigned);
    for (index, _) in remainders {
        if leftover == 0 {
            break;
        }
        if categories[index].tokens > 0 {
            cells[index] += 1;
            leftover -= 1;
        }
    }
    cells
}

/// 按分项顺序铺满 100 格，剩余填空白。
///
/// 参数:
/// - `categories`: 分项
/// - `cells`: 各分项格子数
/// - `styled`: 是否着色
///
/// 返回:
/// - 100 个已着色或纯字符
fn paint_grid(categories: &[CategoryUsage], cells: &[usize], styled: bool) -> Vec<String> {
    let mut grid = Vec::with_capacity(GRID_CELLS);
    for (category, count) in categories.iter().zip(cells) {
        let glyph = if styled {
            format!("{}{FILLED}{RESET}", category.color)
        } else {
            FILLED.to_string()
        };
        grid.extend(std::iter::repeat(glyph).take(*count));
    }
    let empty = if styled {
        format!("{DIM}{EMPTY}{RESET}")
    } else {
        EMPTY.to_string()
    };
    grid.resize(GRID_CELLS, empty);
    grid
}

/// 渲染网格的一行（10 格，格间一空格）。
///
/// 参数:
/// - `grid`: 100 格
/// - `row`: 行号 0..10
///
/// 返回:
/// - 一行网格
fn render_grid_row(grid: &[String], row: usize) -> String {
    let start = row * GRID_COLS;
    grid[start..start + GRID_COLS].join(" ")
}

/// 组装网格右侧的模型信息与分类图例。
///
/// 参数:
/// - `view`: 视图数据
/// - `used`: 已用 token
/// - `window`: 窗口 token
/// - `styled`: 是否着色
///
/// 返回:
/// - 与网格行对齐的右侧文本
fn legend_lines(view: &ContextView, used: usize, window: usize, styled: bool) -> Vec<String> {
    let ratio = used as f32 / window as f32;
    let mut lines = vec![
        format!(
            "{} {}",
            paint(styled, BOLD, &view.model_name),
            paint(
                styled,
                DIM,
                &format!("({} {})", format_tokens(window), t("context", "上下文"))
            )
        ),
        paint(styled, DIM, &view.model_id),
        paint(
            styled,
            DIM,
            &format!(
                "{}/{} {} ({})",
                format_tokens(used),
                format_tokens(window),
                t("tokens", "token"),
                format_percent_int(ratio)
            ),
        ),
        String::new(),
        paint(
            styled,
            &format!("{DIM}{ITALIC}"),
            &t("Estimated usage by category", "按类别估算用量").to_string(),
        ),
    ];
    for category in &view.categories {
        let share = category.tokens as f32 / window as f32;
        let mark = if styled {
            format!("{}{FILLED}{RESET}", category.color)
        } else {
            FILLED.to_string()
        };
        lines.push(format!(
            "{mark} {}: {} {} ({})",
            category.label,
            format_tokens(category.tokens),
            t("tokens", "token"),
            format_percent_frac(share)
        ));
    }
    let free = window.saturating_sub(used);
    let free_mark = if styled {
        format!("{DIM}{EMPTY}{RESET}")
    } else {
        EMPTY.to_string()
    };
    lines.push(format!(
        "{free_mark} {}: {} ({})",
        t("Free space", "剩余空间"),
        format_tokens(free),
        format_percent_frac(free as f32 / window as f32)
    ));
    lines
}

/// 给文本套上 ANSI 前缀；未着色时原样返回。
///
/// 参数:
/// - `styled`: 是否着色
/// - `code`: ANSI 前缀
/// - `text`: 正文
///
/// 返回:
/// - 可能带样式的文本
fn paint(styled: bool, code: &str, text: &str) -> String {
    if styled {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

/// 渲染自动压缩触发说明，方便在 TUI `/context` 里对照设置。
///
/// 参数:
/// - `view`: 上下文视图
///
/// 返回:
/// - 如 `自动压缩 · 90% · 预留 50k · 触发 72k`
fn format_auto_compact_line(view: &ContextView) -> String {
    let percent = (view.compact_ratio * 100.0).round() as u32;
    let scope = if view.session_override {
        t("this session", "本会话")
    } else {
        t("default", "默认")
    };
    if view.compact_reserve == 0 {
        format!(
            "{} · {} · {}% · {} {}",
            t("Auto-compact", "自动压缩"),
            scope,
            percent,
            t("trigger", "触发"),
            format_tokens(view.compact_trigger)
        )
    } else {
        format!(
            "{} · {} · {}% · {} {} · {} {}",
            t("Auto-compact", "自动压缩"),
            scope,
            percent,
            t("reserve", "预留"),
            format_tokens(view.compact_reserve),
            t("trigger", "触发"),
            format_tokens(view.compact_trigger)
        )
    }
}

/// 把 `/context` 参数写进当前会话策略。
///
/// 参数:
/// - `state`: 当前会话状态
/// - `config`: 全局配置，供合并未改动的字段
/// - `update`: 策略改写
///
/// 返回:
/// - 写入是否成功
fn apply_context_policy_update(
    state: &crate::state::StateStore,
    config: &AppConfig,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
) -> Result<()> {
    let Some(update) = update else {
        return Ok(());
    };
    match update {
        crate::control_commands::ContextPolicyUpdate::Reset => state.clear_compaction_policy(),
        crate::control_commands::ContextPolicyUpdate::Set {
            ratio_percent,
            reserve,
        } => {
            let current = state.resolve_compaction_policy(&config.context)?;
            let ratio = crate::config::parse_compaction_ratio_value(ratio_percent as f32 / 100.0);
            let reserve_tokens = reserve.unwrap_or(current.policy.reserve_tokens);
            state.save_compaction_policy(ratio, reserve_tokens)
        }
    }
}

/// 紧凑 token 数：`4.2k` / `1M`。
///
/// 参数:
/// - `value`: token 数
///
/// 返回:
/// - 展示文本
fn format_tokens(value: usize) -> String {
    if value >= 1_000_000 {
        strip_trailing_zero(value as f64 / 1_000_000.0, "M")
    } else if value >= 1_000 {
        strip_trailing_zero(value as f64 / 1_000.0, "k")
    } else {
        value.to_string()
    }
}

/// 去掉一位小数里无意义的 `.0`。
///
/// 参数:
/// - `value`: 缩放后的数值
/// - `suffix`: 单位
///
/// 返回:
/// - 带单位文本
fn strip_trailing_zero(value: f64, suffix: &str) -> String {
    let text = format!("{value:.1}");
    if let Some(whole) = text.strip_suffix(".0") {
        format!("{whole}{suffix}")
    } else {
        format!("{text}{suffix}")
    }
}

/// 标题行百分比取整。
///
/// 参数:
/// - `ratio`: 0~1
///
/// 返回:
/// - 如 `0%`
fn format_percent_int(ratio: f32) -> String {
    format!("{}%", (ratio * 100.0).round().clamp(0.0, 100.0) as i32)
}

/// 分类行百分比保留一位小数。
///
/// 参数:
/// - `ratio`: 0~1
///
/// 返回:
/// - 如 `0.4%`
fn format_percent_frac(ratio: f32) -> String {
    let percent = (ratio * 100.0).clamp(0.0, 100.0);
    if percent > 0.0 && percent < 0.1 {
        "<0.1%".to_string()
    } else {
        format!("{percent:.1}%")
    }
}

#[cfg(test)]
mod tests {
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
}

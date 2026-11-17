mod chart;
mod formatting;
#[cfg(test)]
mod tests;

use crate::i18n::text as t;
use formatting::{paint, tokens};

/// 上下文窗口的单项占用，颜色与柱形图保持一致。
#[derive(Clone, Debug)]
pub(crate) struct CategoryUsage {
    pub label: String,
    pub tokens: usize,
    pub color: (u8, u8, u8),
}

/// 上下文浮层的只读数据，不携带提示词、工具参数或会话秘密。
#[derive(Clone, Debug, Default)]
pub(crate) struct ContextOverview {
    pub used: usize,
    pub window: usize,
    pub categories: Vec<CategoryUsage>,
    pub compact_trigger: usize,
    pub usage: Option<crate::llm::Usage>,
    pub usage_is_turn: bool,
}

impl ContextOverview {
    /// 【上下文】【预算区间】计算未占用空间与压缩预留，二者不会重复计数。
    /// 参数: 无；返回: 空闲量和实际剩余预留量。
    pub(crate) fn free_and_reserved(&self) -> (usize, usize) {
        let used = self.used.min(self.window);
        let trigger = self.compact_trigger.min(self.window);
        let free = trigger.saturating_sub(used);
        (free, self.window.saturating_sub(used).saturating_sub(free))
    }
}

/// 【上下文】【窗口排版】宽终端左右布局，窄终端上下布局，所有文本按显示宽度换行。
/// 参数: 视图数据、可用列数与着色开关；返回: 可直接绘制的 ANSI 行。
pub(crate) fn render(view: &ContextOverview, width: usize, styled: bool) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![format!(
        "{}  {} / {} · {}",
        t("Context", "上下文"),
        tokens(view.used),
        tokens(view.window),
        formatting::percent(view.used, view.window, true)
    )];
    // 1. 【上下文】【极简线性进度条】宽面板顶栏下方使用单轴横线直观反映总占用比例
    if width >= 24 && view.window > 0 {
        lines.push(minimal_bar(view, width.min(52), styled));
    }
    lines.push(String::new());
    let legend = formatting::legend(view, styled);
    let legend_width = legend
        .iter()
        .map(|line| crate::render::ansi_clip::ansi_visible_width(line))
        .max()
        .unwrap_or(0);
    let side_by_side = width >= legend_width + 25;
    let columns = if side_by_side {
        ((width - legend_width - 4) / 2).clamp(10, 20)
    } else {
        ((width + 1) / 2).clamp(1, 20)
    };
    let graph = chart::render(view, columns, 5, styled);
    if side_by_side {
        let graph_width = columns * 2 - 1;
        for row in 0..legend.len().max(graph.len()) {
            let left = graph.get(row).map(String::as_str).unwrap_or("");
            let padding =
                graph_width.saturating_sub(crate::render::ansi_clip::ansi_visible_width(left));
            lines.push(format!(
                "{left}{}    {}",
                " ".repeat(padding),
                legend.get(row).map(String::as_str).unwrap_or("")
            ));
        }
    } else {
        lines.extend(graph);
        lines.push(String::new());
        lines.extend(legend);
    }
    lines.push(String::new());
    lines.extend(pack_segments(&formatting::usage(view), width));
    wrap(
        lines
            .into_iter()
            .map(|line| paint(styled, "", &line))
            .collect(),
        width,
    )
}

/// 【上下文】【极简线性条】使用单轴横线展示总窗口占用，摒弃花哨杂乱符号。
/// 参数: `view` 为上下文概览，`width` 为轨道宽度，`styled` 为是否启用 ANSI 颜色；返回: 单行简约进度条。
fn minimal_bar(view: &ContextOverview, width: usize, styled: bool) -> String {
    let cells = width.max(1);
    let window = view.window.max(1);
    let used = view.used.min(window);
    let filled = if used > 0 {
        ((used as u128 * cells as u128).div_ceil(window as u128) as usize).clamp(1, cells)
    } else {
        0
    };
    let filled_str = "━".repeat(filled);
    let empty_str = "─".repeat(cells - filled);
    if !styled {
        return format!("{filled_str}{empty_str}");
    }
    let palette = crate::render::panel_palette::PanelPalette::current();
    let ratio = used as f64 / window as f64;
    let fill_color = if ratio >= 0.90 {
        palette.removed
    } else if ratio >= 0.72 {
        palette.warning
    } else {
        palette.accent
    };
    let fill_fg = crate::render::panel_palette::fg(fill_color);
    let empty_fg = crate::render::panel_palette::fg(palette.subtle);
    format!("{fill_fg}{filled_str}{empty_fg}{empty_str}\x1b[0m")
}

/// 【上下文】【分段换行】按 ` · ` 分隔符换行，避免把数字拆到两行；单段超宽时交给安全换行处理。
/// 参数: 原始行和列宽；返回: 逻辑行。
fn pack_segments(line: &str, width: usize) -> Vec<String> {
    const SEPARATOR: &str = " · ";
    let mut lines: Vec<String> = Vec::new();
    for segment in line.split(SEPARATOR) {
        match lines.last_mut() {
            Some(last)
                if unicode_width::UnicodeWidthStr::width(last.as_str())
                    + unicode_width::UnicodeWidthStr::width(SEPARATOR)
                    + unicode_width::UnicodeWidthStr::width(segment)
                    <= width =>
            {
                last.push_str(SEPARATOR);
                last.push_str(segment);
            }
            _ => lines.push(segment.to_string()),
        }
    }
    lines
}

/// 【上下文】【安全换行】保留 ANSI 和宽字符边界，避免浮层超出终端宽度。
/// 参数: 逻辑行和列宽；返回: 物理行。
fn wrap(lines: Vec<String>, width: usize) -> Vec<String> {
    lines
        .into_iter()
        .flat_map(|line| {
            crate::render::transcript::AnsiLine::wrap_block(&line, width)
                .into_iter()
                .map(|line| line.as_str().to_string())
        })
        .collect()
}

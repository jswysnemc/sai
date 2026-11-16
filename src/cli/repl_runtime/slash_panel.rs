//! 独立于输入框的斜杠命令面板：支持全量命令滚动视口、分类标签与自适应高度。

#[path = "slash_panel/render.rs"]
mod render;

use crate::cli::repl_commands::{
    repl_command_suggestions, ReplCommandSuggestion, MAX_REPL_COMMAND_SUGGESTIONS,
};
use crate::cli::repl_text::visible_width;

/// 斜杠面板默认最大总行数（1 行顶栏 + 8 条建议）
const DEFAULT_MAX_PANEL_ROWS: usize = MAX_REPL_COMMAND_SUGGESTIONS + 1;

/// 独立于输入框的斜杠命令面板。
///
/// 视觉对齐：`PanelPalette` 前景层次与细线顶栏（无实心底色遮挡正文），
/// 支持超长命令列表滚动浏览与响应式高度收缩。
pub(super) struct SlashPanel {
    suggestions: Vec<ReplCommandSuggestion>,
    selected: usize,
    max_rows: usize,
}

impl SlashPanel {
    /// 根据当前输入构造斜杠命令面板。
    ///
    /// 参数:
    /// - `input`: 当前输入文本
    /// - `selected`: 当前选中项
    /// - `streaming`: 模型是否正在运行；为 true 时打断类命令标记为禁用
    ///
    /// 返回:
    /// - 已过滤且支持滚动的命令面板
    pub(super) fn new(input: &str, selected: usize, streaming: bool) -> Self {
        let suggestions = repl_command_suggestions(input, streaming);
        let selected = selected.min(suggestions.len().saturating_sub(1));
        Self {
            suggestions,
            selected,
            max_rows: DEFAULT_MAX_PANEL_ROWS,
        }
    }

    /// 判断面板是否需要展示。
    ///
    /// 返回:
    /// - 是否存在匹配命令
    pub(super) fn is_visible(&self) -> bool {
        !self.suggestions.is_empty()
    }

    /// 【终端】【面板高度】按当前可用视口高度限制命令面板行数，至少保留一行展示选中项。
    ///
    /// 参数:
    /// - `rows`: 允许面板占用的最大行数
    ///
    /// 返回:
    /// - 无
    pub(super) fn limit_height(&mut self, rows: usize) {
        self.max_rows = self.max_rows.min(rows.max(1));
    }

    /// 计算是否显示紧凑顶栏及可见命令条目数。
    ///
    /// 返回:
    /// - `(是否包含顶栏, 可见建议行数)`
    fn window_geometry(&self) -> (bool, usize) {
        if self.suggestions.is_empty() {
            return (false, 0);
        }
        let show_header = self.max_rows >= 2 && self.suggestions.len() > 1;
        let item_budget = if show_header {
            self.max_rows - 1
        } else {
            self.max_rows
        }
        .min(MAX_REPL_COMMAND_SUGGESTIONS)
        .min(self.suggestions.len())
        .max(1);
        (show_header, item_budget)
    }

    /// 返回面板占用的终端行数。
    ///
    /// 返回:
    /// - 包含顶栏与可见命令行的总高度
    pub(super) fn height(&self) -> u16 {
        let (show_header, items) = self.window_geometry();
        (items + usize::from(show_header)).min(u16::MAX as usize) as u16
    }

    /// 返回面板各行的渲染结果。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 面板每行的完整文本
    pub(super) fn rendered_lines(&self, cols: usize) -> Vec<String> {
        let (show_header, visible_items) = self.window_geometry();
        if visible_items == 0 {
            return Vec::new();
        }
        // 1. 【终端】【滚动窗口】保持当前选中命令处于窗口视野中央
        let start = self
            .selected
            .saturating_sub(visible_items / 2)
            .min(self.suggestions.len().saturating_sub(visible_items));
        let cmd_width = self.suggestions[start..start + visible_items]
            .iter()
            .map(|item| visible_width(item.command))
            .max()
            .unwrap_or(14)
            .saturating_add(2);
        let mut lines = Vec::with_capacity(visible_items + usize::from(show_header));
        if show_header {
            lines.push(render::header_line(
                start,
                visible_items,
                self.suggestions.len(),
                cols,
            ));
        }
        for (offset, suggestion) in self
            .suggestions
            .iter()
            .skip(start)
            .take(visible_items)
            .enumerate()
        {
            let index = start + offset;
            lines.push(render::format_suggestion(
                *suggestion,
                cmd_width,
                cols,
                index == self.selected,
            ));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 置灰命令整行灰化，且不能套用选中态的亮白。
    #[test]
    fn disabled_suggestion_is_greyed_out_even_when_selected() {
        let suggestion = ReplCommandSuggestion {
            command: "/model",
            description: "choose the active model",
            disabled: true,
        };
        let selected = render::format_suggestion(suggestion, 18, 72, true);
        assert!(
            selected.contains("\x1b[90m"),
            "disabled entry should be grey: {selected:?}"
        );
        assert!(
            !selected.contains("\x1b[97m"),
            "disabled entry must not use the selectable bright white: {selected:?}"
        );
        let plain = crate::render::activity_animation::strip_ansi_for_test(&selected);
        assert!(
            plain.contains('/') && !plain.contains("choose the active model"),
            "description should be replaced by the wait hint: {plain}"
        );
    }

    /// 选中行不得铺实心背景，并使用 `→` 指示当前项。
    #[test]
    fn selected_suggestion_uses_arrow_without_background_fill() {
        let suggestion = ReplCommandSuggestion {
            command: "/model",
            description: "choose the active model",
            disabled: false,
        };
        let selected = render::format_suggestion(suggestion, 18, 72, true);
        let plain = crate::render::activity_animation::strip_ansi_for_test(&selected);
        assert!(plain.starts_with('→'), "selected should use arrow: {plain}");
        assert!(
            !selected.contains("48;5;") && !selected.contains("48;2;") && !selected.contains("[K"),
            "selected must not paint a solid background bar: {selected:?}"
        );
        assert!(
            selected.contains("\x1b[97m"),
            "selected command should be bright white"
        );
    }

    /// 未选中行保持弱化样式，前缀为空位以对齐选中箭头。
    #[test]
    fn unselected_suggestion_is_dim_and_aligned() {
        let suggestion = ReplCommandSuggestion {
            command: "/help",
            description: "show available commands",
            disabled: false,
        };
        let line = render::format_suggestion(suggestion, 18, 72, false);
        let plain = crate::render::activity_animation::strip_ansi_for_test(&line);
        assert!(
            plain.starts_with("  /help") || plain.starts_with(' '),
            "{plain}"
        );
        assert!(!line.contains("48;5;") && !line.contains("48;2;"));
        assert!(line.contains("\x1b[2m"));
    }

    /// 【斜杠面板测试】【滚动浏览】超出一页的命令列表可以通过选择索引顺畅滚动并保留窗口范围提示。
    #[test]
    fn slash_panel_scrolls_beyond_first_page_and_shows_range() {
        let panel = SlashPanel::new("/", 15, false);
        let lines = panel.rendered_lines(80);
        let header = crate::render::activity_animation::strip_ansi_for_test(&lines[0]);
        assert!(header.contains('/'), "顶栏应展示位置范围: {header}");
        assert!(
            lines
                .iter()
                .map(|line| crate::render::activity_animation::strip_ansi_for_test(line))
                .any(|plain| plain.starts_with('→')),
            "滚动窗口内必须始终包含选中指示"
        );
    }
}

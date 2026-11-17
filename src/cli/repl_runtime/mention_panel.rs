use crate::cli::repl_mentions::MentionSuggestion;
use crate::cli::repl_text::visible_width;
use crate::render::panel_palette::{fg, PanelPalette};

/// 引用面板最多同时展示的候选行数，完整列表通过导航浏览。
const MAX_VISIBLE_MENTION_ROWS: usize = 8;
const RESET: &str = "\x1b[0m";

/// `#` skill 与 `@` 文件引用的过滤面板。
pub(super) struct MentionPanel {
    suggestions: Vec<MentionSuggestion>,
    selected: usize,
    max_rows: usize,
}

impl MentionPanel {
    /// 【终端】【引用面板】根据建议列表构造引用面板。
    /// 参数: `suggestions` 为已过滤建议，`selected` 为当前选中项；返回: 引用面板实例。
    pub(super) fn new(suggestions: Vec<MentionSuggestion>, selected: usize) -> Self {
        let selected = selected.min(suggestions.len().saturating_sub(1));
        Self {
            suggestions,
            selected,
            max_rows: MAX_VISIBLE_MENTION_ROWS,
        }
    }

    /// 【终端】【引用面板】判断面板是否需要展示。
    /// 参数: 无；返回: 存在匹配项时为真。
    pub(super) fn is_visible(&self) -> bool {
        !self.suggestions.is_empty()
    }

    /// 【终端】【引用面板】返回面板占用的终端行数。
    /// 参数: 无；返回: 可见建议数量。
    pub(super) fn height(&self) -> u16 {
        self.suggestions.len().min(self.max_rows) as u16
    }

    /// 【终端】【引用面板】按实际可用高度收缩显示窗口，保留全部候选。
    /// 参数: `rows` 为可用行数；返回: 无，存在候选时至少保留一行。
    pub(super) fn limit_height(&mut self, rows: usize) {
        self.max_rows = self.max_rows.min(rows.max(1));
    }

    /// 【终端】【引用面板】返回面板各行的渲染结果。
    /// 参数: `cols` 为终端列数；返回: 面板每行文本。
    #[allow(dead_code)]
    pub(super) fn rendered_lines(&self, cols: usize) -> Vec<String> {
        self.rendered_rows_with_meta(cols).0
    }

    /// 【终端】【引用面板】渲染候选行并返回每行对应的候选下标，供鼠标点击与滚轮交互使用。
    /// 参数: `cols` 为终端列数；返回: `(面板文本行, 每行对应的候选信息(下标, 是否禁用))`。
    pub(super) fn rendered_rows_with_meta(
        &self,
        cols: usize,
    ) -> (Vec<String>, Vec<Option<(usize, bool)>>) {
        // 1. 【终端】【引用面板】显示窗口跟随选中项移动，首尾不越过候选范围
        let visible_rows = usize::from(self.height());
        let start = self
            .selected
            .saturating_sub(visible_rows / 2)
            .min(self.suggestions.len().saturating_sub(visible_rows));
        let mut lines = Vec::with_capacity(visible_rows);
        let mut meta = Vec::with_capacity(visible_rows);
        for (index, suggestion) in self
            .suggestions
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
        {
            lines.push(format_suggestion(suggestion, cols, index == self.selected));
            meta.push(Some((index, false)));
        }
        (lines, meta)
    }
}

/// 【终端】【引用条目】对齐输入框第 3 列正文基线，按显示列宽补齐标签并使用主题自适应色。
/// 参数: `suggestion` 为标签与说明，`cols` 为终端列数，`selected` 为是否当前选中；返回: 不超宽的 ANSI 行。
fn format_suggestion(suggestion: &MentionSuggestion, cols: usize, selected: bool) -> String {
    let palette = PanelPalette::current();
    // 1. 【终端】【基线对齐】前缀恒占 3 列（" → " 或 "   "），与输入框提示符完全对齐
    let command_width = 24usize.min(cols.saturating_sub(4));
    let description_width = cols.saturating_sub(command_width + 4);
    let label = pad_right(
        &truncate_to_width(&suggestion.label, command_width),
        command_width,
    );
    let description = truncate_to_width(&suggestion.description, description_width);
    // 2. 【终端】【稳定间距】选中与未选中行统一保留单空格列间距，消除切换时的水平跳动
    if selected {
        return format!(
            " {}→{RESET} \x1b[1m{}{label}{RESET} {}{description}{RESET}",
            fg(palette.accent),
            fg(palette.text),
            fg(palette.text),
        );
    }
    format!(
        "   \x1b[2m{}{label}{RESET} \x1b[2m{}{description}{RESET}",
        fg(palette.muted),
        fg(palette.muted),
    )
}

/// 【终端】【列宽补齐】按显示列宽在右侧补齐空格，避免中文文件名或技能名撑爆列宽。
/// 参数: `value` 为已截断文本，`target` 为目标显示列数；返回: 恰好达到目标列数的字符串。
fn pad_right(value: &str, target: usize) -> String {
    let width = visible_width(value);
    if width >= target {
        value.to_string()
    } else {
        format!("{value}{}", " ".repeat(target - width))
    }
}

/// 【终端】【宽度裁剪】将纯文本截断到指定终端显示宽度。
/// 参数: `value` 为原始文本，`width` 为最大显示宽度；返回: 截断后的文本。
fn truncate_to_width(value: &str, width: usize) -> String {
    if visible_width(value) <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        let char_width = visible_width(&ch.to_string());
        if used.saturating_add(char_width) > width - 3 {
            break;
        }
        output.push(ch);
        used = used.saturating_add(char_width);
    }
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ansi_clip::strip_ansi;

    /// 【引用面板测试】【中文对齐】验证中文文件名按显示宽度补齐，不越界且选中前后说明列零跳动。
    /// 参数: 无；返回: 无。
    #[test]
    fn cjk_labels_stay_within_columns_and_align_across_selection() {
        let item = MentionSuggestion {
            insert: "@文档说明.md".into(),
            label: "@文档说明.md".into(),
            description: "项目架构说明文档".into(),
            continue_filter: false,
        };
        let active = format_suggestion(&item, 48, true);
        let idle = format_suggestion(&item, 48, false);
        assert!(!active.contains("\x1b[97m"));
        assert!(visible_width(&active) <= 48);
        assert!(visible_width(&idle) <= 48);
        let active_plain = strip_ansi(&active);
        let idle_plain = strip_ansi(&idle);
        assert!(active_plain.starts_with(" → @文档说明.md"));
        assert!(idle_plain.starts_with("   @文档说明.md"));
        assert_eq!(
            visible_width(active_plain.split("项目").next().unwrap()),
            visible_width(idle_plain.split("项目").next().unwrap())
        );
    }
}

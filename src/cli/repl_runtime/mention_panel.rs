use crate::cli::repl_mentions::MentionSuggestion;
use crate::cli::repl_text::visible_width;

/// 引用面板最多同时展示的候选行数，完整列表通过导航浏览
const MAX_VISIBLE_MENTION_ROWS: usize = 8;

/// `#` skill 与 `@` 文件引用的过滤面板。
pub(super) struct MentionPanel {
    suggestions: Vec<MentionSuggestion>,
    selected: usize,
    max_rows: usize,
}

impl MentionPanel {
    /// 根据建议列表构造引用面板。
    ///
    /// 参数:
    /// - `suggestions`: 已过滤建议
    /// - `selected`: 当前选中项
    ///
    /// 返回:
    /// - 引用面板
    pub(super) fn new(suggestions: Vec<MentionSuggestion>, selected: usize) -> Self {
        let selected = selected.min(suggestions.len().saturating_sub(1));
        Self {
            suggestions,
            selected,
            max_rows: MAX_VISIBLE_MENTION_ROWS,
        }
    }

    /// 判断面板是否需要展示。
    ///
    /// 返回:
    /// - 存在匹配项时为真
    pub(super) fn is_visible(&self) -> bool {
        !self.suggestions.is_empty()
    }

    /// 返回面板占用的终端行数。
    ///
    /// 返回:
    /// - 可见建议数量
    pub(super) fn height(&self) -> u16 {
        self.suggestions.len().min(self.max_rows) as u16
    }

    /// 【终端】【引用面板】按实际可用高度收缩显示窗口，保留全部候选
    /// 参数: `rows` 为可用行数；返回: 无，存在候选时至少保留一行
    pub(super) fn limit_height(&mut self, rows: usize) {
        self.max_rows = self.max_rows.min(rows.max(1));
    }

    /// 返回面板各行的渲染结果。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 面板每行文本
    pub(super) fn rendered_lines(&self, cols: usize) -> Vec<String> {
        // 1. 【终端】【引用面板】显示窗口跟随选中项移动，首尾不越过候选范围
        let visible_rows = usize::from(self.height());
        let start = self
            .selected
            .saturating_sub(visible_rows / 2)
            .min(self.suggestions.len().saturating_sub(visible_rows));
        self.suggestions
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
            .map(|(index, suggestion)| format_suggestion(suggestion, cols, index == self.selected))
            .collect()
    }
}

/// 格式化引用面板的一条建议。
///
/// 参数:
/// - `suggestion`: 标签与说明
/// - `cols`: 终端列数
/// - `selected`: 是否为当前选中项
///
/// 返回:
/// - 不超过终端宽度的面板行
fn format_suggestion(suggestion: &MentionSuggestion, cols: usize, selected: bool) -> String {
    let marker = if selected { "→" } else { " " };
    let command_width = 24usize.min(cols.saturating_sub(3));
    let description_width = cols.saturating_sub(command_width + 3);
    let description = truncate_to_width(&suggestion.description, description_width);
    if selected {
        return format!(
            "{marker} \x1b[97m{:<command_width$}\x1b[0m {}",
            truncate_to_width(&suggestion.label, command_width),
            description
        );
    }
    format!(
        "{marker} \x1b[2m{:<command_width$}\x1b[0m\x1b[2m{}\x1b[0m",
        truncate_to_width(&suggestion.label, command_width),
        description
    )
}

/// 将文本截断到指定终端宽度。
///
/// 参数:
/// - `value`: 原始文本
/// - `width`: 最大显示宽度
///
/// 返回:
/// - 截断后的文本
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

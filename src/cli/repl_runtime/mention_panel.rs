use crate::cli::repl_mentions::MentionSuggestion;
use crate::cli::repl_text::visible_width;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io::Write;

/// `#` skill 与 `@` 文件引用的过滤面板。
pub(super) struct MentionPanel {
    suggestions: Vec<MentionSuggestion>,
    selected: usize,
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
    /// - 建议数量
    pub(super) fn height(&self) -> u16 {
        self.suggestions.len().min(u16::MAX as usize) as u16
    }

    /// 返回面板各行的渲染结果。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 面板每行文本
    pub(super) fn rendered_lines(&self, cols: usize) -> Vec<String> {
        self.suggestions
            .iter()
            .enumerate()
            .map(|(index, suggestion)| format_suggestion(suggestion, cols, index == self.selected))
            .collect()
    }

    /// 在输入框下方绘制引用面板。
    ///
    /// 参数:
    /// - `output`: 终端输出
    /// - `top`: 面板顶部行号
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 绘制是否成功
    pub(super) fn draw<W: Write>(&self, output: &mut W, top: u16, cols: usize) -> Result<()> {
        for (index, suggestion) in self.suggestions.iter().enumerate() {
            queue!(
                output,
                MoveTo(0, top.saturating_add(index as u16)),
                Print(format_suggestion(suggestion, cols, index == self.selected))
            )?;
        }
        Ok(())
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

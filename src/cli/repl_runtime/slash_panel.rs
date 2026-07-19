use crate::cli::repl_commands::{visible_repl_command_suggestions, ReplCommandSuggestion};
use crate::cli::repl_text::visible_width;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io::Write;

/// 独立于输入框的斜杠命令面板。
pub(super) struct SlashPanel {
    suggestions: Vec<ReplCommandSuggestion>,
    selected: usize,
}

impl SlashPanel {
    /// 根据当前输入构造斜杠命令面板。
    ///
    /// 参数:
    /// - `input`: 当前输入文本
    /// - `selected`: 当前选中项
    ///
    /// 返回:
    /// - 已过滤的命令面板
    pub(super) fn new(input: &str, selected: usize) -> Self {
        let suggestions = visible_repl_command_suggestions(input);
        let selected = selected.min(suggestions.len().saturating_sub(1));
        Self {
            suggestions,
            selected,
        }
    }

    /// 判断面板是否需要展示。
    ///
    /// 返回:
    /// - 是否存在匹配命令
    pub(super) fn is_visible(&self) -> bool {
        !self.suggestions.is_empty()
    }

    /// 返回面板占用的终端行数。
    ///
    /// 返回:
    /// - 命令建议数量
    pub(super) fn height(&self) -> u16 {
        self.suggestions.len().min(u16::MAX as usize) as u16
    }

    /// 在输入框下方绘制命令面板。
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
                Print(format_suggestion(*suggestion, cols, index == self.selected,))
            )?;
        }
        Ok(())
    }
}

/// 格式化斜杠面板的一条命令建议。
///
/// 参数:
/// - `suggestion`: 命令与说明
/// - `cols`: 终端列数
/// - `selected`: 是否为当前选中项
///
/// 返回:
/// - 不超过终端宽度的面板行
fn format_suggestion(suggestion: ReplCommandSuggestion, cols: usize, selected: bool) -> String {
    let command_width = 18usize.min(cols.saturating_sub(3));
    let description_width = cols.saturating_sub(command_width + 3);
    let description = truncate_to_width(suggestion.description, description_width);
    if selected {
        return format!(
            "\x1b[48;5;238m  \x1b[1m{:<command_width$}\x1b[0m\x1b[48;5;238m\x1b[2m{}\x1b[0m\x1b[48;5;238m\x1b[K\x1b[0m",
            suggestion.command, description
        );
    }
    format!(
        "  \x1b[1m{:<command_width$}\x1b[0m\x1b[2m{}\x1b[0m",
        suggestion.command, description
    )
}

/// 将说明文本截断到指定终端宽度。
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

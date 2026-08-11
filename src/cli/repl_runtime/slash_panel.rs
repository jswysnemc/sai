use crate::cli::repl_commands::{visible_repl_command_suggestions, ReplCommandSuggestion};
use crate::cli::repl_text::visible_width;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use std::io::Write;

/// 独立于输入框的斜杠命令面板。
///
/// 视觉对齐参考：无实心底色（透出下方 transcript），选中用 `→` + 亮白，
/// 未选中用弱化灰蓝，左右两列分别为命令与说明。
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

    /// 返回面板各行的渲染结果。
    ///
    /// 供 composer 计算内容签名：选中项变化会改变行文本，
    /// 据此判断能否跳过重绘。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 面板每行的完整文本
    pub(super) fn rendered_lines(&self, cols: usize) -> Vec<String> {
        self.suggestions
            .iter()
            .enumerate()
            .map(|(index, suggestion)| {
                format_suggestion(*suggestion, cols, index == self.selected)
            })
            .collect()
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
/// 不用实心背景色铺满行——终端里那会变成黑灰色块，挡住下方 transcript。
/// 选中态用 `→` + 亮白，未选中用等宽空位 + 弱化色，左右两列对齐命令与说明。
///
/// 参数:
/// - `suggestion`: 命令与说明
/// - `cols`: 终端列数
/// - `selected`: 是否为当前选中项
///
/// 返回:
/// - 不超过终端宽度的面板行
fn format_suggestion(suggestion: ReplCommandSuggestion, cols: usize, selected: bool) -> String {
    let marker = if selected { "→" } else { " " };
    // 标记 1 列 + 间隔 1 列 + 命令列 + 间隔 1 列 + 说明
    let command_width = 18usize.min(cols.saturating_sub(3));
    let description_width = cols.saturating_sub(command_width + 3);
    let description = truncate_to_width(suggestion.description, description_width);
    if selected {
        // 亮白命令 + 常规亮度说明，无 48; 背景、无 EL 填色
        return format!(
            "{marker} \x1b[97m{:<command_width$}\x1b[0m {}",
            suggestion.command, description
        );
    }
    format!(
        "{marker} \x1b[2m{:<command_width$}\x1b[0m\x1b[2m{}\x1b[0m",
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 选中行不得铺实心背景，并使用 `→` 指示当前项。
    #[test]
    fn selected_suggestion_uses_arrow_without_background_fill() {
        let suggestion = ReplCommandSuggestion {
            command: "/model",
            description: "choose the active model",
        };
        let selected = format_suggestion(suggestion, 72, true);
        let plain = crate::render::activity_animation::strip_ansi_for_test(&selected);
        assert!(plain.starts_with('→'), "selected should use arrow: {plain}");
        assert!(
            !selected.contains("48;5;") && !selected.contains("48;2;") && !selected.contains("[K"),
            "selected must not paint a solid background bar: {selected:?}"
        );
        assert!(selected.contains("\x1b[97m"), "selected command should be bright white");
    }

    /// 未选中行保持弱化样式，前缀为空位以对齐选中箭头。
    #[test]
    fn unselected_suggestion_is_dim_and_aligned() {
        let suggestion = ReplCommandSuggestion {
            command: "/help",
            description: "show available commands",
        };
        let line = format_suggestion(suggestion, 72, false);
        let plain = crate::render::activity_animation::strip_ansi_for_test(&line);
        assert!(plain.starts_with("  /help") || plain.starts_with(' '), "{plain}");
        assert!(!line.contains("48;5;"));
        assert!(line.contains("\x1b[2m"));
    }
}

use super::line::AnsiLine;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// REPL 启动时显示的会话基础信息。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WelcomeCell {
    pub(crate) version: String,
    pub(crate) model: String,
    pub(crate) directory: String,
    pub(crate) permissions: String,
}

/// 按当前终端宽度渲染 Codex 风格的启动面板。
///
/// 参数:
/// - `cell`: 启动信息 source
/// - `width`: 终端列数
///
/// 返回:
/// - 不需要再次换行的 ANSI 行
pub(crate) fn display_lines(cell: &WelcomeCell, width: usize) -> Vec<AnsiLine> {
    let inner_width = width.saturating_sub(2).clamp(24, 72);
    let title = format!("Sai (v{})", cell.version);
    let title = truncate_to_width(&title, inner_width.saturating_sub(4));
    let title_width = UnicodeWidthStr::width(title.as_str());
    let top_padding = inner_width.saturating_sub(title_width + 3);
    let top = format!(
        "\x1b[2m╭─\x1b[0m \x1b[1m{title}\x1b[0m \x1b[2m{}╮\x1b[0m",
        "─".repeat(top_padding)
    );
    let model = panel_row("model:", &cell.model, Some("/model to change"), inner_width);
    let directory = panel_row("directory:", &cell.directory, None, inner_width);
    let permissions = panel_row("permissions:", &cell.permissions, None, inner_width);
    let bottom = format!("\x1b[2m╰{}╯\x1b[0m", "─".repeat(inner_width));

    vec![
        AnsiLine::new(top),
        AnsiLine::new(model),
        AnsiLine::new(directory),
        AnsiLine::new(permissions),
        AnsiLine::new(bottom),
    ]
}

/// 构造带边框且不超过面板宽度的一行信息。
///
/// 参数:
/// - `label`: 字段标签
/// - `value`: 字段值
/// - `hint`: 可选提示
/// - `inner_width`: 面板内部宽度
///
/// 返回:
/// - 带 ANSI 边框的单行文本
fn panel_row(label: &str, value: &str, hint: Option<&str>, inner_width: usize) -> String {
    let label_width = UnicodeWidthStr::width(label);
    let hint_width = hint.map(UnicodeWidthStr::width).unwrap_or(0);
    let value_width = inner_width
        .saturating_sub(label_width)
        .saturating_sub(hint_width)
        .saturating_sub(if hint.is_some() { 3 } else { 2 });
    let value = truncate_to_width(value, value_width);
    let content = match hint {
        Some(hint) => format!(" {label} {value}  \x1b[2m{hint}\x1b[0m"),
        None => format!(" {label} {value}"),
    };
    let padding = inner_width.saturating_sub(visible_width(&content));
    format!(
        "\x1b[2m│\x1b[0m{content}{}\x1b[2m│\x1b[0m",
        " ".repeat(padding)
    )
}

/// 将文本截断到指定显示宽度。
///
/// 参数:
/// - `value`: 原始文本
/// - `width`: 最大显示宽度
///
/// 返回:
/// - 不超过最大宽度的文本
fn truncate_to_width(value: &str, width: usize) -> String {
    if UnicodeWidthStr::width(value) <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let mut output = String::new();
    let mut used = 0usize;
    for ch in value.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(char_width) > width - 3 {
            break;
        }
        output.push(ch);
        used = used.saturating_add(char_width);
    }
    output.push_str("...");
    output
}

/// 计算含 ANSI 样式文本的显示宽度。
///
/// 参数:
/// - `value`: 带 ANSI 样式的文本
///
/// 返回:
/// - 可见字符的显示宽度
fn visible_width(value: &str) -> usize {
    let mut width = 0usize;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if ('@'..='~').contains(&next) {
                    break;
                }
            }
            continue;
        }
        width = width.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
    }
    width
}

#[cfg(test)]
mod tests {
    use super::{display_lines, WelcomeCell};

    /// 验证启动面板包含会话关键字段并适配窄终端。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn welcome_panel_contains_runtime_details() {
        let cell = WelcomeCell {
            version: "0.1.4".to_string(),
            model: "gpt-5".to_string(),
            directory: "/workspace".to_string(),
            permissions: "YOLO mode".to_string(),
        };

        let lines = display_lines(&cell, 48);

        assert_eq!(lines.len(), 5);
        assert!(lines.iter().any(|line| line.as_str().contains("gpt-5")));
        assert!(lines
            .iter()
            .any(|line| line.as_str().contains("permissions:")));
    }
}

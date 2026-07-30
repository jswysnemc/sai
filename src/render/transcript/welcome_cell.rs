use super::line::AnsiLine;
use crate::render::brand_logo::{logo_lines, LOGO_HEIGHT, LOGO_WIDTH};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 品牌标志使用的实心块样式（与 Web 端 --signal 同色）。
const LOGO_STYLE: &str = "\x1b[38;2;58;114;100m";
/// 标志与右侧信息面板之间的间隔列数。
const LOGO_PANEL_GAP: usize = 2;
/// 展示标志所需的最小终端宽度，低于该宽度只渲染信息面板。
const LOGO_MIN_TERMINAL_WIDTH: usize = 52;

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
/// 宽终端在信息面板左侧并排展示品牌标志；窄终端优先保证信息完整，省略标志。
///
/// 参数:
/// - `cell`: 启动信息 source
/// - `width`: 终端列数
///
/// 返回:
/// - 不需要再次换行的 ANSI 行
pub(crate) fn display_lines(cell: &WelcomeCell, width: usize) -> Vec<AnsiLine> {
    // 1. 预留标志占用的列宽后再计算面板内部宽度
    let show_logo = width >= LOGO_MIN_TERMINAL_WIDTH;
    let logo_reserved = if show_logo { LOGO_WIDTH + LOGO_PANEL_GAP } else { 0 };
    let panel = panel_lines(cell, width.saturating_sub(logo_reserved));
    if !show_logo {
        return panel.into_iter().map(AnsiLine::new).collect();
    }

    // 2. 标志与面板行数一致，逐行左右拼接
    let logo = logo_lines(LOGO_STYLE);
    let gap = " ".repeat(LOGO_PANEL_GAP);
    let blank_logo = " ".repeat(LOGO_WIDTH);
    (0..panel.len().max(LOGO_HEIGHT))
        .map(|index| {
            let logo_row = logo.get(index).cloned().unwrap_or_else(|| blank_logo.clone());
            match panel.get(index) {
                Some(panel_row) => AnsiLine::new(format!("{logo_row}{gap}{panel_row}")),
                None => AnsiLine::new(logo_row),
            }
        })
        .collect()
}

/// 渲染不含品牌标志的信息面板。
///
/// 参数:
/// - `cell`: 启动信息 source
/// - `width`: 面板可用列数
///
/// 返回:
/// - 带 ANSI 边框的面板行
fn panel_lines(cell: &WelcomeCell, width: usize) -> Vec<String> {
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

    vec![top, model, directory, permissions, bottom]
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

    /// 【终端】【品牌标志】验证宽终端并排展示标志，窄终端优先保留信息。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn welcome_panel_shows_logo_only_on_wide_terminals() {
        let cell = WelcomeCell {
            version: "0.1.4".to_string(),
            model: "gpt-5".to_string(),
            directory: "/workspace".to_string(),
            permissions: "YOLO mode".to_string(),
        };

        let narrow = display_lines(&cell, 48);
        assert!(
            narrow.iter().all(|line| !line.as_str().contains('█')),
            "窄终端不应渲染标志"
        );
        assert!(narrow[0].as_str().starts_with("\x1b[2m╭"));

        let wide = display_lines(&cell, 100);
        assert_eq!(wide.len(), 5);
        assert!(
            wide.iter().filter(|line| line.as_str().contains('█')).count() == 5,
            "宽终端每行都应带标志列"
        );
        // 标志位于信息面板左侧
        for line in &wide {
            let text = line.as_str();
            let logo_at = text.find('█');
            let panel_at = text.find('╭').or_else(|| text.find('│')).or_else(|| text.find('╰'));
            if let (Some(logo_at), Some(panel_at)) = (logo_at, panel_at) {
                assert!(logo_at < panel_at, "标志必须在面板左侧");
            }
        }
        assert!(wide.iter().any(|line| line.as_str().contains("gpt-5")));
    }
}

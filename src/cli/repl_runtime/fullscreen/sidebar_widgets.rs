//! 无背景色的定宽标题、条目与变更统计。

use super::sidebar_palette::{fg, Rgb, SidebarPalette};
use super::sidebar_style::clip;
use crate::cli::repl_text::visible_width;

/// 【终端侧栏】【安全文字】清除外部文本控制字符，保留单行内容。
/// 参数: text 为外部文本；返回: 可显示文字。
pub(super) fn safe_text(text: &str) -> String {
    crate::render::ansi_clip::strip_ansi(text)
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}

/// 【终端侧栏】【状态标签】只使用前景色，不铺背景或额外边框。
/// 参数: label 为标签，color 为颜色；返回: ANSI 标签。
pub(super) fn badge(label: &str, color: Rgb) -> String {
    format!("{}{label}\x1b[39m", fg(color))
}

/// 【终端侧栏】【固定标题】标题和计数占一行，不随区域正文滚动。
/// 参数: title 为标题，count 为计数，collapsed 为折叠状态，width 为宽度；返回: 标题行。
pub(super) fn heading(title: &str, count: &str, collapsed: bool, width: usize) -> String {
    let marker = if collapsed { "▸" } else { "▾" };
    let palette = SidebarPalette::current();
    let left = format!("{}\x1b[1m{marker} {title}\x1b[22m", fg(palette.text));
    format!("{}\x1b[0m", columns(&left, count, width))
}

/// 【终端侧栏】【内容条目】按列宽裁剪内容，次要文本使用前景弱化。
/// 参数: text 为内容，width 为宽度，muted 为次要状态；返回: 定宽条目。
pub(super) fn item(text: &str, width: usize, muted: bool) -> String {
    let palette = SidebarPalette::current();
    let color = fg(if muted { palette.muted } else { palette.text });
    let text = clip(text, width).replace("\x1b[39m", &color);
    format!(
        "{color}{text}{}\x1b[0m",
        " ".repeat(width.saturating_sub(visible_width(&text)))
    )
}

/// 【终端侧栏】【交互反馈】选中加粗，悬停反色，保留增删行各自的颜色。
/// 参数: text 为 ANSI 行，selected 为选中状态；返回: 状态样式行。
pub(super) fn highlight(text: &str, selected: bool) -> String {
    let style = if selected { "\x1b[1m" } else { "\x1b[7m" };
    format!(
        "{style}{}\x1b[0m",
        text.replace("\x1b[0m", &format!("\x1b[0m{style}"))
    )
}

/// 【终端侧栏】【双列排版】左侧名称和右侧统计之间填充留白，优先保留统计。
/// 参数: left/right 为内容，width 为总宽；返回: 定宽行。
pub(super) fn columns(left: &str, right: &str, width: usize) -> String {
    let right = clip(right, width);
    let left = clip(
        left,
        width.saturating_sub(visible_width(&right) + usize::from(!right.is_empty())),
    );
    format!(
        "{left}{}{right}",
        " ".repeat(width.saturating_sub(visible_width(&left) + visible_width(&right)))
    )
}

/// 【终端审查】【增删统计】新增使用绿色加号，删除使用红色减号，未知统计不伪装成零。
/// 参数: stats 为可选文件统计；返回: 着色的增删数量。
pub(super) fn changes(stats: Option<&crate::web::GitFileStats>) -> String {
    let palette = SidebarPalette::current();
    let (added, removed) = match stats {
        Some(stats) if !stats.binary => (stats.added.to_string(), stats.removed.to_string()),
        _ => ("?".into(), "?".into()),
    };
    format!(
        "{}+{added} {}-{removed}\x1b[39m",
        fg(palette.added),
        fg(palette.removed)
    )
}

/// 【终端显示】【移除背景】保留前景与文字效果，只移除设置背景色的 SGR 参数。
/// 参数: text 为 ANSI 文本；返回: 不含背景色设置的文本。
#[cfg(test)]
pub(in crate::cli::repl_runtime) fn without_background(text: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < text.len() {
        if text.as_bytes()[index] != 0x1b {
            let ch = text[index..].chars().next().unwrap();
            output.push(ch);
            index += ch.len_utf8();
            continue;
        }
        let end = crate::render::terminal_image::escape_sequence_end(text, index)
            .max(index + 1)
            .min(text.len());
        let sequence = &text[index..end];
        if let Some(params) = sequence
            .strip_prefix("\x1b[")
            .and_then(|value| value.strip_suffix('m'))
        {
            let parts = params.split(';').collect::<Vec<_>>();
            let mut kept = Vec::new();
            let mut part = 0;
            while part < parts.len() {
                let code = parts[part].parse::<u16>().unwrap_or(0);
                let size = if matches!(code, 38 | 48 | 58) {
                    match parts.get(part + 1) {
                        Some(&"5") => 3,
                        Some(&"2") => 5,
                        _ => 1,
                    }
                } else {
                    1
                };
                if code != 48 && !(40..=47).contains(&code) && !(100..=107).contains(&code) {
                    kept.extend_from_slice(&parts[part..(part + size).min(parts.len())]);
                }
                part += size;
            }
            if !kept.is_empty() {
                output.push_str(&format!("\x1b[{}m", kept.join(";")));
            }
        } else {
            output.push_str(sequence);
        }
        index = end;
    }
    output
}

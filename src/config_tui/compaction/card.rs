//! 演示风格的紧凑圆角卡片与小窗口降级。

use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::transcript::AnsiLine;

pub(super) const ACCENT: &str = "\x1b[38;2;170;232;246m";
pub(super) const BRAND: &str = "\x1b[1m\x1b[38;2;94;196;168m";
pub(super) const BORDER: &str = "\x1b[38;2;90;110;118m";
pub(super) const MUTED: &str = "\x1b[38;2;140;160;168m";
pub(super) const SELECTED: &str = "\x1b[48;2;36;64;72m\x1b[1m";
pub(super) const USED: &str = "\x1b[38;2;110;210;165m";
pub(super) const TRIGGER: &str = "\x1b[38;2;245;166;110m";
pub(super) const RESET: &str = "\x1b[0m";

/// 【上下文】【数值缩写】按数量级缩写窗口和预留，原始数值仍用于计算。
/// 参数：token 数；返回：简短标签。
pub(super) fn compact(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 && value.is_multiple_of(1_000) {
        format!("{}k", value / 1_000)
    } else {
        value.to_string()
    }
}

/// 【上下文】【边框排版】在边线上嵌入标题或快捷键，不超出给定列宽。
/// 参数：文本、宽度和底边标记；返回：带样式边框。
fn edge(text: &str, width: usize, bottom: bool) -> String {
    let text = clip_ansi_to_width(text, width.saturating_sub(6), "");
    let fill = width.saturating_sub(ansi_visible_width(&text) + 5);
    let (left, right) = if bottom {
        ('╰', '╯')
    } else {
        ('╭', '╮')
    };
    format!(
        "{BORDER}{left}─ {RESET}{text}{BORDER} {}{right}{RESET}",
        "─".repeat(fill)
    )
}

/// 【上下文】【内容边框】为正文或快捷键补齐内侧空格并绘制统一侧边。
/// 参数：text 为单行 ANSI 内容，inner 为正文列数；返回：带左右边框的行。
fn row(text: &str, inner: usize) -> String {
    format!(
        "{BORDER}│{RESET} {text}{RESET}{} {BORDER}│{RESET}",
        " ".repeat(inner.saturating_sub(ansi_visible_width(text)))
    )
}

/// 【上下文】【快捷键换行】按完整操作分组折行，避免窄屏切断按键名称和说明。
/// 参数：help 为以间隔点分组的帮助，width 为正文宽度；返回：不越界的帮助行。
fn help_rows(help: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut current = String::new();
    for group in help.split(" · ") {
        let candidate = if current.is_empty() {
            group.to_string()
        } else {
            format!("{current} · {group}")
        };
        if ansi_visible_width(&candidate) <= width {
            current = candidate;
            continue;
        }
        if !current.is_empty() {
            rows.push(std::mem::take(&mut current));
        }
        if ansi_visible_width(group) > width {
            rows.extend(
                AnsiLine::wrap_block(group, width)
                    .into_iter()
                    .map(|line| line.as_str().to_string()),
            );
        } else {
            current = group.to_string();
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows
}

/// 【上下文】【卡片组合】宽屏使用圆角边框，极窄屏退回无边框文本。
/// 参数：标题、正文、帮助与宽度；返回：可直接绘制的逐行文本。
pub(super) fn frame(title: &str, body: &[String], help: &str, width: usize) -> Vec<String> {
    let framed = width >= 16;
    let inner = width.saturating_sub(if framed { 4 } else { 0 }).max(1);
    let mut lines = vec![if framed {
        edge(title, width, false)
    } else {
        clip_ansi_to_width(title, width, "")
    }];
    for source in body {
        for line in AnsiLine::wrap_block(source, inner) {
            let text = line.as_str();
            lines.push(if framed {
                row(text, inner)
            } else {
                clip_ansi_to_width(text, width, "")
            });
        }
    }
    if framed && ansi_visible_width(help) + 6 <= width {
        lines.push(edge(&format!("{MUTED}{help}{RESET}"), width, true));
    } else {
        for text in help_rows(help, inner) {
            lines.push(if framed {
                row(&format!("{MUTED}{text}{RESET}"), inner)
            } else {
                clip_ansi_to_width(&text, width, "")
            });
        }
        if framed {
            lines.push(format!("{BORDER}╰{}╯{RESET}", "─".repeat(width - 2)));
        }
    }
    lines
}

/// 【上下文】【矮屏布局】高度不足时保留模式控件和退出说明，避免越界写屏。
/// 参数：完整卡片和可用行数；返回：裁剪后的文本行。
pub(super) fn fit_height(mut lines: Vec<String>, rows: usize) -> Vec<String> {
    if lines.len() > rows {
        lines.truncate(rows);
        if let Some(last) = lines.last_mut() {
            let width = ansi_visible_width(last);
            let help = crate::i18n::text("Enter save · Esc cancel", "Enter 保存 · Esc 取消");
            *last = if width >= 16 {
                edge(help, width, true)
            } else {
                clip_ansi_to_width(help, width, "")
            };
        }
    }
    lines
}

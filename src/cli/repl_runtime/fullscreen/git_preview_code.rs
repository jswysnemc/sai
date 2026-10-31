//! 差异代码的语法缓存、固定行号和 ANSI 安全横向滚动。

use super::git_preview_patch::{LineKind, PatchLine, PreviewContent};
use super::sidebar_palette::{bg, fg, SidebarPalette};
use crate::render::ansi_clip::{ansi_visible_width, clip_ansi_to_width};
use crate::render::syntect_highlight::{paint, Carry};
use std::path::Path;

const MAX_SYNTAX_BYTES: usize = 200_000;
/// 行号四列、增删标记一列和两列间距。
pub(super) const GUTTER_WIDTH: usize = 7;

/// 【终端审查】【行号列宽】超过四位的行号扩展排版栏，鼠标坐标与实际绘制保持一致。
/// 参数: line 为差异行；返回: 行号、标记和间距合计列数。
pub(super) fn gutter_width(line: &PatchLine) -> usize {
    GUTTER_WIDTH
        + line
            .new
            .or(line.old)
            .map_or(0, |number| number.to_string().len().saturating_sub(4))
}

/// 【终端审查】【语法缓存】按扩展名复用现有着色器，原文件和新文件分别保存解析状态。
/// 参数: content 为差异模型，path 为文件路径；返回: 无，缓存写入 content.code。
pub(super) fn cache(content: &mut PreviewContent, path: &str) {
    let language = Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let (mut old, mut new): (Option<Box<Carry>>, Option<Box<Carry>>) = (None, None);
    let mut bytes = 0usize;
    content.code = content
        .lines
        .iter()
        .map(|line| {
            // 1. 【终端审查】【片段边界】不同差异块之间缺失源码，不继承上个块的语法状态
            if matches!(line.kind, LineKind::Hunk | LineKind::Notice) {
                old = None;
                new = None;
                return line.text.clone();
            }
            bytes = bytes.saturating_add(line.text.len());
            if bytes > MAX_SYNTAX_BYTES {
                return line.text.clone();
            }
            // 2. 【终端审查】【版本隔离】上下文同时推进两个版本，增删只推进所属版本
            let source = format!("{}\n", line.text);
            let previous = if line.kind != LineKind::Added {
                paint(language, &source, &mut old)
            } else {
                None
            };
            let updated = if line.kind != LineKind::Removed {
                paint(language, &source, &mut new)
            } else {
                None
            };
            updated
                .or(previous)
                .map(|text| text.replace('\n', ""))
                .unwrap_or_else(|| line.text.clone())
        })
        .collect();
}

/// 【终端审查】【代码行】删除使用红底、新增使用绿底，行号、增删标记与语法前景保留。
/// 参数: line 为差异行，code 为语法缓存，horizontal 为代码偏移，width 为净宽；返回: 定宽 ANSI 行。
pub(super) fn render(
    line: &PatchLine,
    code: Option<&str>,
    horizontal: usize,
    width: usize,
) -> String {
    let palette = SidebarPalette::current();
    if line.kind == LineKind::Hunk {
        let title = format!("  {}", line.text);
        let rule = "─".repeat(width.saturating_sub(ansi_visible_width(&title) + 1));
        return fill_row(&format!("{title} {rule}"), &fg(palette.accent), "", width);
    }
    if line.kind == LineKind::Notice {
        return fill_row(&line.text, &fg(palette.muted), "", width);
    }
    let (marker, color) = match line.kind {
        LineKind::Added => ("+", palette.added),
        LineKind::Removed => ("-", palette.removed),
        _ => (" ", palette.text),
    };
    let number = line
        .new
        .or(line.old)
        .map_or_else(String::new, |value| value.to_string());
    let gutter = if line.kind == LineKind::Context {
        palette.muted
    } else {
        color
    };
    let background = match line.kind {
        LineKind::Added => bg(palette.added_bg),
        LineKind::Removed => bg(palette.removed_bg),
        _ => String::new(),
    };
    let base = fg(palette.text);
    let text = skip_columns(code.unwrap_or(&line.text), horizontal);
    fill_row(
        &format!("{}{number:>4} {marker} {base}{text}", fg(gutter)),
        &base,
        &background,
        width,
    )
}

/// 【终端审查】【整行填色】语法复位和裁剪后恢复行背景，填满右栏净宽并在行尾闭合样式。
/// 参数: text 为安全 ANSI 行，foreground/background 为基础样式，width 为净宽；返回: 定宽行。
fn fill_row(text: &str, foreground: &str, background: &str, width: usize) -> String {
    let base = format!("{background}{foreground}");
    // 1. 【终端审查】【语法复位】词元重置不能清除所属差异行的红绿背景
    let text = text.replace("\x1b[0m", &format!("\x1b[0m{base}"));
    let clipped = clip_ansi_to_width(&format!("{base}{text}"), width, "");
    // 2. 【终端审查】【边界填充】裁剪可能插入重置，重新设置背景后填满剩余列
    let padding = width.saturating_sub(ansi_visible_width(&clipped));
    format!("{clipped}{base}{}\x1b[0m", " ".repeat(padding))
}

/// 【终端审查】【代码偏移】仅跳过可见字符，保留偏移前生效的语法控制序列。
/// 参数: text 为安全 ANSI 代码，columns 为列偏移；返回: 保持语法样式的剩余代码。
pub(super) fn skip_columns(text: &str, columns: usize) -> String {
    let mut output = String::new();
    let mut width = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap();
        if ch == '\x1b' {
            let end = crate::render::terminal_image::escape_sequence_end(text, index)
                .max(index + 1)
                .min(text.len());
            output.push_str(&text[index..end]);
            index = end;
            continue;
        }
        let size = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width >= columns {
            output.push(ch);
        } else if width + size > columns {
            // 1. 【终端审查】【宽字符边界】跨偏移边界的双列字符使用剩余列空格，避免列号漂移
            output.push_str(&" ".repeat(width + size - columns));
        }
        width = width.saturating_add(size);
        index += ch.len_utf8();
    }
    output
}

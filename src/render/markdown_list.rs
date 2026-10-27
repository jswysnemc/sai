use super::markdown_inline::InlineMathMode;
use super::style::{MD_LIST_MARKER_STYLE, RESET};

#[derive(Clone)]
struct Level {
    source_indent: usize,
    content_column: usize,
}

/// 【终端】【列表上下文】按源码缩进记录层级与正文列，续行不退回项目符号列。
#[derive(Clone, Default)]
pub(super) struct ListLayout {
    levels: Vec<Level>,
    multiline: bool,
}

impl ListLayout {
    /// 【终端】【列表上下文】遇到独立代码、表格等块时结束列表上下文。
    /// 参数: 无；返回: 无。
    pub(super) fn reset(&mut self) {
        self.levels.clear();
        self.multiline = false;
    }

    /// 【终端】【列表布局】渲染列表项和显式续行，复杂同级组之间保留一行留白。
    /// 参数: `line` 为源码行，`mode` 为公式模式，`separated` 为源码是否有空行；返回: 列表行或空值。
    pub(super) fn render(
        &mut self,
        line: &str,
        mode: InlineMathMode,
        separated: bool,
    ) -> Option<String> {
        let trimmed = line.trim_start();
        let indent = line[..line.len() - trimmed.len()]
            .replace('\t', "    ")
            .len();
        if super::markdown_blocks::is_horizontal_rule(trimmed) || trimmed.starts_with(['#', '>']) {
            self.reset();
            return None;
        }
        if let Some((source_marker, body)) = parse_item(trimmed) {
            let previous_depth = self.levels.len();
            self.levels.retain(|level| level.source_indent < indent);
            let depth = self.levels.len();
            let gap = !separated
                && depth == 0
                && previous_depth > 0
                && (previous_depth > 1 || self.multiline);
            let marker = if source_marker.len() == 1 {
                ["-", "·", "–"][depth % 3]
            } else {
                source_marker
            };
            self.levels.push(Level {
                source_indent: indent,
                content_column: indent + marker.chars().count() + 1,
            });
            self.multiline = false;
            return Some(format!(
                "{}{}{MD_LIST_MARKER_STYLE}{marker}{RESET} {}",
                if gap { "\n" } else { "" },
                " ".repeat(indent),
                super::markdown::render_inline_for_mode(body, mode)
            ));
        }
        let level = self.levels.last()?;
        if separated && indent < level.content_column {
            self.reset();
            return None;
        }
        self.multiline = true;
        Some(format!(
            "{}{MD_LIST_MARKER_STYLE}{RESET}{}",
            " ".repeat(level.content_column),
            super::markdown::render_inline_for_mode(trimmed, mode)
        ))
    }
}

/// 【终端】【列表识别】识别无序和有序项目，数字标记保留实际位数。
/// 参数: `line` 为去除缩进的源码；返回: 原始标记及正文。
fn parse_item(line: &str) -> Option<(&str, &str)> {
    if matches!(line.as_bytes().first(), Some(b'-' | b'*' | b'+'))
        && line.as_bytes().get(1) == Some(&b' ')
    {
        return Some((&line[..1], line[2..].trim_start()));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && matches!(line.as_bytes().get(digits), Some(b'.' | b')'))
        && line.as_bytes().get(digits + 1) == Some(&b' ')
    {
        Some((&line[..digits + 1], line[digits + 2..].trim_start()))
    } else {
        None
    }
}

/// 【终端】【列表单行】为无上下文预览保留相同标记格式，层级按缩进推导。
/// 参数: `line` 为源码，`mode` 为公式模式；返回: 列表行或空值。
pub(super) fn render_line(line: &str, mode: InlineMathMode) -> Option<String> {
    let trimmed = line.trim_start();
    parse_item(trimmed)?;
    let indent = line.len() - trimmed.len();
    let mut layout = ListLayout::default();
    for source_indent in (0..indent).step_by(2) {
        layout.levels.push(Level {
            source_indent,
            content_column: source_indent + 2,
        });
    }
    layout.render(line, mode, false)
}

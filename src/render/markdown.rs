use crate::render::asset_block;
use crate::render::code_block::{highlight_code_line, render_code_footer, render_code_header};
use crate::render::markdown_blocks;
pub(crate) use crate::render::markdown_inline::render_inline;
#[cfg(test)]
pub(crate) use crate::render::markdown_inline::render_table_cell;
#[cfg(test)]
pub(crate) use crate::render::markdown_inline::render_table_cell_content;
use crate::render::markdown_inline::{render_inline_with_math_mode, InlineMathMode};
use crate::render::streaming_asset_block::StreamingAssetBlock;
use crate::render::style::{HEADER_STYLE, RESET, TERTIARY_STYLE};
use crate::render::table;
use crate::render::table::streaming::StreamingTable;

pub(crate) struct MarkdownStreamRenderer {
    buffer: String,
    line_renderer: MarkdownLineRenderer,
}

impl MarkdownStreamRenderer {
    /// 创建流式 Markdown 渲染器。
    ///
    /// 返回:
    /// - 新的流式渲染器
    pub(crate) fn new() -> Self {
        Self::with_table_replacement(true)
    }

    /// 创建用于 source 重放的 Markdown 渲染器。
    ///
    /// 表格在闭合前保持缓冲，避免生成仅适用于实时终端的光标回退序列。
    ///
    /// 返回:
    /// - 可稳定重放的 Markdown 渲染器
    pub(crate) fn new_stable() -> Self {
        Self::with_table_replacement(false)
    }

    /// 创建用于 source-backed 实时尾部的 Markdown 渲染器。
    ///
    /// 表格在流式阶段保留原始 Markdown，定稿 history cell 再替换为计算结果。
    ///
    /// 返回:
    /// - 不生成光标回退序列的实时预览渲染器
    pub(crate) fn new_source_preview() -> Self {
        Self {
            buffer: String::new(),
            line_renderer: MarkdownLineRenderer::new_source_preview(),
        }
    }

    /// 根据表格是否允许替换原始行创建行渲染器。
    ///
    /// 参数:
    /// - `replace_streamed_table_rows`: 是否生成流式表格替换控制序列
    ///
    /// 返回:
    /// - 初始化后的 Markdown 渲染器
    fn with_table_replacement(replace_streamed_table_rows: bool) -> Self {
        Self {
            buffer: String::new(),
            line_renderer: MarkdownLineRenderer::new(replace_streamed_table_rows),
        }
    }

    /// 推入流式 Markdown 增量。
    ///
    /// 参数:
    /// - `delta`: 新收到的 Markdown 文本片段
    ///
    /// 返回:
    /// - 已完整渲染的终端文本
    pub(crate) fn push(&mut self, delta: &str) -> String {
        self.buffer.push_str(delta);
        let mut output = String::new();
        while let Some(index) = self.buffer.find('\n') {
            let line = self.buffer[..index].to_string();
            self.buffer = self.buffer[index + 1..].to_string();
            output.push_str(&self.line_renderer.render_line(&line));
        }
        output
    }

    /// 刷新剩余 Markdown 缓冲。
    ///
    /// 返回:
    /// - 最后一段渲染文本
    pub(crate) fn flush(&mut self) -> String {
        let mut output = String::new();
        if !self.buffer.is_empty() {
            let line = std::mem::take(&mut self.buffer);
            output.push_str(&self.line_renderer.render_line(&line));
        }
        output.push_str(&self.line_renderer.flush());
        output
    }
}

struct MarkdownLineRenderer {
    in_code_block: bool,
    in_math_block: bool,
    code_lang: String,
    code_buffer: Vec<String>,
    code_is_asset: bool,
    just_closed_code_block: bool,
    pending_blank_lines: usize,
    math_buffer: Vec<String>,
    table: StreamingTable,
    asset_block: StreamingAssetBlock,
    inline_math_mode: InlineMathMode,
}

impl MarkdownLineRenderer {
    /// 创建按行 Markdown 渲染器。
    ///
    /// 返回:
    /// - 新的按行渲染器
    fn new(replace_streamed_table_rows: bool) -> Self {
        Self {
            in_code_block: false,
            in_math_block: false,
            code_lang: String::new(),
            code_buffer: Vec::new(),
            code_is_asset: false,
            just_closed_code_block: false,
            pending_blank_lines: 0,
            math_buffer: Vec::new(),
            table: if replace_streamed_table_rows {
                StreamingTable::new()
            } else {
                StreamingTable::new_stable()
            },
            asset_block: if replace_streamed_table_rows {
                StreamingAssetBlock::new()
            } else {
                StreamingAssetBlock::new_stable()
            },
            inline_math_mode: InlineMathMode::TerminalImage,
        }
    }

    /// 创建 source-backed 实时预览行渲染器。
    ///
    /// 返回:
    /// - 表格保留原文的行渲染器
    fn new_source_preview() -> Self {
        let mut renderer = Self::new(false);
        renderer.table = StreamingTable::new_source_preview();
        renderer.asset_block = StreamingAssetBlock::new_source_preview();
        renderer.inline_math_mode = InlineMathMode::Source;
        renderer
    }

    /// 渲染单行 Markdown。
    ///
    /// 参数:
    /// - `line`: 单行 Markdown 文本
    ///
    /// 返回:
    /// - 当前可输出的终端文本
    fn render_line(&mut self, line: &str) -> String {
        let skip_empty = std::mem::take(&mut self.just_closed_code_block);
        if skip_empty && !self.in_code_block && line.trim().is_empty() {
            return String::new();
        }

        if line.trim_start().starts_with("```") {
            if self.in_code_block {
                self.in_code_block = false;
                let lang = std::mem::take(&mut self.code_lang);
                let lines = std::mem::take(&mut self.code_buffer);
                if self.code_is_asset {
                    let raw_close = self.asset_block.push_line(line);
                    let rendered = asset_block::render_asset_block(&lang, &lines);
                    raw_close + &self.asset_block.finish(rendered)
                } else {
                    self.just_closed_code_block = true;
                    render_code_footer(&lines)
                }
            } else {
                self.pending_blank_lines = 0;
                let pending = self.flush();
                self.in_code_block = true;
                self.code_lang = line
                    .trim_start()
                    .trim_start_matches('`')
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                self.code_is_asset = asset_block::is_asset_language(&self.code_lang);
                self.code_buffer.clear();
                if self.code_is_asset {
                    self.asset_block.reset();
                    pending + &self.asset_block.push_line(line)
                } else {
                    pending + &render_code_header(&self.code_lang)
                }
            }
        } else if self.in_code_block {
            self.code_buffer.push(line.to_string());
            if self.code_is_asset {
                self.asset_block.push_line(line)
            } else {
                format!("{}\n", highlight_code_line(&self.code_lang, line))
            }
        } else if line.trim().is_empty() {
            let output = if self.table.is_active() {
                self.table.finish()
            } else {
                String::new()
            };
            self.pending_blank_lines += 1;
            output
        } else if line.trim() == "$$" {
            if self.in_math_block {
                self.in_math_block = false;
                let raw_close = self.asset_block.push_line(line);
                let rendered = asset_block::render_math_block(&self.math_buffer);
                self.math_buffer.clear();
                raw_close + &self.asset_block.finish(rendered)
            } else {
                let pending = self.flush();
                self.in_math_block = true;
                self.math_buffer.clear();
                self.asset_block.reset();
                pending + &self.asset_block.push_line(line)
            }
        } else if self.in_math_block {
            self.math_buffer.push(line.to_string());
            self.asset_block.push_line(line)
        } else if table::looks_like_table_row(line) {
            self.pending_blank_lines = 0;
            self.table.push_line(line)
        } else {
            self.pending_blank_lines = 0;
            let mut output = self.flush();
            let rendered = match self.inline_math_mode {
                InlineMathMode::TerminalImage => render_markdown_line(line),
                InlineMathMode::Source => {
                    render_markdown_line_with_math_mode(line, self.inline_math_mode)
                }
            };
            output.push_str(&rendered);
            output.push('\n');
            output
        }
    }

    /// 刷新行级渲染器缓冲。
    ///
    /// 返回:
    /// - 缓冲区渲染结果
    fn flush(&mut self) -> String {
        if self.in_code_block {
            self.in_code_block = false;
            let lang = std::mem::take(&mut self.code_lang);
            let lines = std::mem::take(&mut self.code_buffer);
            if self.code_is_asset {
                let rendered = asset_block::render_asset_block(&lang, &lines);
                self.asset_block.finish(rendered)
            } else {
                render_code_footer(&lines)
            }
        } else if self.in_math_block {
            self.in_math_block = false;
            let rendered = asset_block::render_math_block(&self.math_buffer);
            self.math_buffer.clear();
            self.asset_block.finish(rendered)
        } else if !self.table.is_active() {
            self.take_pending_blank_lines()
        } else if self.table.is_confirmed() {
            let mut output = self.table.finish();
            output.push_str(&self.take_pending_blank_lines());
            output
        } else {
            self.table.finish();
            self.take_pending_blank_lines()
        }
    }

    /// 取出待输出空行。
    ///
    /// 返回:
    /// - 空行文本
    fn take_pending_blank_lines(&mut self) -> String {
        let count = std::mem::take(&mut self.pending_blank_lines);
        "\n".repeat(count)
    }
}

/// 渲染单行 Markdown 文本。
///
/// 参数:
/// - `line`: 原始 Markdown 行
///
/// 返回:
/// - 渲染后的终端文本，不包含结尾换行
pub(crate) fn render_markdown_line(line: &str) -> String {
    render_markdown_line_with_math_mode(line, InlineMathMode::TerminalImage)
}

/// 按指定公式策略渲染单行 Markdown 文本。
///
/// 参数:
/// - `line`: 原始 Markdown 行
/// - `math_mode`: 行内公式渲染策略
///
/// 返回:
/// - 渲染后的终端文本，不包含结尾换行
fn render_markdown_line_with_math_mode(line: &str, math_mode: InlineMathMode) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    if let Some(header) = render_header(trimmed, math_mode) {
        return header;
    }
    if let Some((depth, rest)) = parse_blockquote(trimmed) {
        let bars = "\x1b[32m| \x1b[0m".repeat(depth);
        return format!(
            "{indent}{bars}\x1b[32m{}\x1b[0m",
            render_inline_for_mode(rest, math_mode)
        );
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return format!(
            "{indent}{TERTIARY_STYLE}-{RESET} {}",
            render_inline_for_mode(rest, math_mode)
        );
    }
    let digits = trimmed.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits > 0
        && trimmed.as_bytes().get(digits) == Some(&b'.')
        && trimmed.as_bytes().get(digits + 1) == Some(&b' ')
    {
        let marker = &trimmed[..=digits];
        let rest = &trimmed[digits + 2..];
        return format!(
            "{indent}{TERTIARY_STYLE}{marker}{RESET} {}",
            render_inline_for_mode(rest, math_mode)
        );
    }
    if markdown_blocks::is_horizontal_rule(trimmed) {
        return markdown_blocks::horizontal_rule();
    }
    render_inline_for_mode(line, math_mode)
}

/// 根据公式策略选择行内渲染入口。
///
/// 参数:
/// - `text`: 原始行内文本
/// - `math_mode`: 行内公式渲染策略
///
/// 返回:
/// - 带 ANSI 样式的行内文本
fn render_inline_for_mode(text: &str, math_mode: InlineMathMode) -> String {
    match math_mode {
        InlineMathMode::TerminalImage => render_inline(text),
        InlineMathMode::Source => render_inline_with_math_mode(text, math_mode),
    }
}

/// 解析 Markdown 引用层级。
///
/// 参数:
/// - `line`: 原始行
///
/// 返回:
/// - 引用层级和剩余文本
fn parse_blockquote(line: &str) -> Option<(usize, &str)> {
    let mut depth = 0;
    let mut rest = line;
    while let Some(stripped) = rest.strip_prefix('>') {
        depth += 1;
        rest = stripped.strip_prefix(' ').unwrap_or(stripped);
    }
    (depth > 0).then_some((depth, rest))
}

/// 渲染 Markdown 标题。
///
/// 参数:
/// - `line`: 去除缩进后的行
///
/// 返回:
/// - 标题渲染结果
fn render_header(line: &str, math_mode: InlineMathMode) -> Option<String> {
    let level = line.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 || line.as_bytes().get(level) != Some(&b' ') {
        return None;
    }
    let prefix = "#".repeat(level);
    Some(format!(
        "{HEADER_STYLE}{prefix} {}{RESET}",
        render_inline_for_mode(&line[level + 1..], math_mode)
    ))
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;

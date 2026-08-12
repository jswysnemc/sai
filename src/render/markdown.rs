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
use crate::render::style::{
    FOOTNOTE_DEF_STYLE, MD_H1_STYLE, MD_H2_STYLE, MD_H3_STYLE, MD_LIST_MARKER_STYLE,
    MD_QUOTE_BAR_STYLE, RESET,
};
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

    /// 创建用于全量重绘表面（TUI live）的 Markdown 渲染器。
    ///
    /// 表格在推入时缓冲；调用方在每次重绘末尾通过 `snapshot_open_structures`
    /// 取当前最优列宽预览。闭合后 `finish` 输出最终表格，无光标回退序列。
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

    /// 预览尚未闭合的表格等结构（全量重绘路径使用，不破坏缓冲）。
    ///
    /// 返回:
    /// - 当前最优渲染；无开放结构时为空
    pub(crate) fn snapshot_open_structures(&self) -> String {
        self.line_renderer.snapshot_open()
    }
}

struct MarkdownLineRenderer {
    in_code_block: bool,
    in_math_block: bool,
    code_lang: String,
    code_buffer: Vec<String>,
    code_is_asset: bool,
    pending_blank_lines: usize,
    has_emitted_content: bool,
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
            pending_blank_lines: 0,
            has_emitted_content: false,
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

    /// 创建全量重绘用的行渲染器。
    ///
    /// 返回:
    /// - 表格走 snapshot 模式的行渲染器
    fn new_source_preview() -> Self {
        let mut renderer = Self::new(false);
        renderer.table = StreamingTable::new_source_preview();
        // 资产块闭合即出图（渲染结果按内容缓存，重绘走 Kitty 替换语义）；
        // 未闭合部分由 snapshot_open 以弱化源码预览
        renderer.asset_block = StreamingAssetBlock::new_stable();
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
        let output = self.render_line_inner(line);
        if !output.is_empty() {
            self.has_emitted_content = true;
        }
        output
    }

    /// 按行类型分派渲染。
    ///
    /// 参数:
    /// - `line`: 单行 Markdown 文本
    ///
    /// 返回:
    /// - 当前可输出的终端文本
    fn render_line_inner(&mut self, line: &str) -> String {
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
                    render_code_footer(&lines)
                }
            } else {
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
            let gap = self.take_pending_blank_lines();
            gap + &self.table.push_line(line)
        } else {
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
        } else {
            let mut output = self.table.finish();
            output.push_str(&self.take_pending_blank_lines());
            output
        }
    }

    /// 非破坏性预览开放中的表格与资产块。
    ///
    /// 返回:
    /// - 预览文本；无开放结构时为空
    fn snapshot_open(&self) -> String {
        if self.table.is_active() {
            return self.table.snapshot();
        }
        // 未闭合的 mermaid / 公式块：以弱化源码预览，闭合后由 finish 出图
        if (self.in_code_block && self.code_is_asset) || self.in_math_block {
            let buffer = if self.in_math_block {
                &self.math_buffer
            } else {
                &self.code_buffer
            };
            let label = if self.in_math_block {
                "math"
            } else {
                self.code_lang.as_str()
            };
            let mut output = format!("\x1b[2m┌─ {label} (rendering…)\x1b[0m\n");
            for line in buffer.iter().take(5) {
                output.push_str(&format!("\x1b[2m│ {line}\x1b[0m\n"));
            }
            if buffer.len() > 5 {
                output.push_str("\x1b[2m│ …\x1b[0m\n");
            }
            output.push_str("\x1b[2m└─\x1b[0m\n");
            return output;
        }
        String::new()
    }

    /// 取出块间空行（统一节奏：相邻块之间恰好一行）。
    ///
    /// 源中连续多个空行压缩为一行；首个块之前的空行直接丢弃，
    /// 避免正文以空行开头（区块前距由 transcript cell 统一负责）。
    ///
    /// 返回:
    /// - 空行文本（零或一行）
    fn take_pending_blank_lines(&mut self) -> String {
        let count = std::mem::take(&mut self.pending_blank_lines);
        if count == 0 || !self.has_emitted_content {
            return String::new();
        }
        "\n".to_string()
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
    if let Some((label, rest)) = parse_footnote_definition(trimmed) {
        // 脚注定义：标记加粗着色作为锚点，正文按行内规则渲染
        return format!(
            "{indent}{FOOTNOTE_DEF_STYLE}[{label}]{RESET} {}",
            render_inline_for_mode(rest, math_mode)
        );
    }
    if let Some((depth, rest)) = parse_blockquote(trimmed) {
        // 引用条：左侧弱化细竖条（嵌套层级叠加）+ dim 正文；
        // 行内样式的 reset 会中断 dim，重置后补回保持整行统一
        let bars = "▏".repeat(depth);
        let body =
            render_inline_for_mode(rest, math_mode).replace(RESET, &format!("{RESET}\x1b[2m"));
        return format!("{indent}{MD_QUOTE_BAR_STYLE}{bars}{RESET} \x1b[2m{body}\x1b[0m");
    }
    if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        return format!(
            "{indent}{MD_LIST_MARKER_STYLE}-{RESET} {}",
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
            "{indent}{MD_LIST_MARKER_STYLE}{marker}{RESET} {}",
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

/// 解析脚注定义行。
///
/// 参数:
/// - `line`: 去除缩进后的行
///
/// 返回:
/// - 命中时返回 `(标签, 正文)`；标签不含 `^` 前缀
fn parse_footnote_definition(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("[^")?;
    let close = rest.find("]:")?;
    let label = &rest[..close];
    if !crate::render::markdown_inline::is_footnote_label(label) {
        return None;
    }
    let body = rest[close + 2..].trim_start();
    Some((label, body))
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
/// 去掉 `#` 前缀噪音，层级靠字重区分：H1 加粗下划线、H2 加粗、
/// H3 及以下加粗弱化；行内样式的 reset 会中断标题样式，重置后补回。
///
/// 参数:
/// - `line`: 去除缩进后的行
/// - `math_mode`: 行内公式渲染策略
///
/// 返回:
/// - 标题渲染结果
fn render_header(line: &str, math_mode: InlineMathMode) -> Option<String> {
    let level = line.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 || line.as_bytes().get(level) != Some(&b' ') {
        return None;
    }
    let style = match level {
        1 => MD_H1_STYLE,
        2 => MD_H2_STYLE,
        _ => MD_H3_STYLE,
    };
    let body = render_inline_for_mode(&line[level + 1..], math_mode)
        .replace(RESET, &format!("{RESET}{style}"));
    Some(format!("{style}{body}{RESET}"))
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;

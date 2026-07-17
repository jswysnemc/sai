use super::commands::{command_available, ensure_file_exists, run_command};
use super::math::{build_fallback_svg, convert_svg_to_png, render_image, try_render_ratex};
use super::MathRenderMode;
use crate::render::style::{INLINE_CODE_STYLE, RESET};
use crate::render::table::CellContent;
use crate::render::terminal_image;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// 将行内公式渲染为单行半块文本。
///
/// 参数:
/// - `source`: 数学公式源码
///
/// 返回:
/// - 半块文本，失败时返回带样式源码
pub(crate) fn render_inline_halfblock(source: &str) -> String {
    if source.trim().is_empty() {
        return String::new();
    }
    match render_inline_halfblock_inner(source) {
        Ok(rendered) => rendered,
        Err(_) => format!("{INLINE_CODE_STYLE}{source}{RESET}"),
    }
}

/// 生成行内公式图片并转换为单行半块文本。
///
/// 参数:
/// - `source`: 数学公式源码
///
/// 返回:
/// - 单行半块文本
fn render_inline_halfblock_inner(source: &str) -> Result<String> {
    let temp_dir = tempfile::tempdir().context("failed to create temporary render directory")?;
    let png = render_image(source, &temp_dir, MathRenderMode::Inline)?;
    terminal_image::render_halfblock_line(&png, 8)
}

/// 将表格公式单元格渲染为终端图片协议。
///
/// 参数:
/// - `source`: 纯公式或文字与公式混合内容
/// - `max_cols`: 最大终端列宽
/// - `mixed`: 是否包含普通文本
///
/// 返回:
/// - 图片单元格，失败时返回源码单元格
pub(crate) fn render_cell(source: &str, max_cols: usize, mixed: bool) -> CellContent {
    if source.trim().is_empty() {
        return CellContent::empty();
    }
    match render_cell_inner(source, max_cols, mixed) {
        Ok(mut content) => {
            content.math_source = Some(encode_source(source, mixed));
            content
        }
        Err(_) => CellContent::from_inline(format!("{INLINE_CODE_STYLE}{source}{RESET}")),
    }
}

/// 编码表格公式源，供列宽变化后重新渲染。
///
/// 参数:
/// - `source`: 原始内容
/// - `mixed`: 是否包含普通文本
///
/// 返回:
/// - 带类型前缀的源码
fn encode_source(source: &str, mixed: bool) -> String {
    if mixed {
        format!("mixed:{source}")
    } else {
        format!("pure:{source}")
    }
}

/// 解码表格公式源。
///
/// 参数:
/// - `encoded`: 带类型前缀的源码
///
/// 返回:
/// - 原始内容与混合标记
pub(crate) fn decode_source(encoded: &str) -> (String, bool) {
    if let Some(rest) = encoded.strip_prefix("mixed:") {
        return (rest.to_string(), true);
    }
    if let Some(rest) = encoded.strip_prefix("pure:") {
        return (rest.to_string(), false);
    }
    (encoded.to_string(), false)
}

/// 生成公式 PNG 并转换为表格图片单元格。
///
/// 参数:
/// - `source`: 公式或混合内容
/// - `max_cols`: 最大列宽
/// - `mixed`: 是否包含普通文本
///
/// 返回:
/// - 图片单元格
fn render_cell_inner(source: &str, max_cols: usize, mixed: bool) -> Result<CellContent> {
    let temp_dir = tempfile::tempdir().context("failed to create temporary render directory")?;
    let png = if mixed {
        render_mixed_image(source, &temp_dir)?
    } else {
        render_image(source, &temp_dir, MathRenderMode::Inline)?
    };
    let term_cols = crossterm::terminal::size()
        .map(|(cols, _)| usize::from(cols))
        .unwrap_or(80);
    let default_max = (term_cols.saturating_sub(10) * 2 / 5).clamp(8, 36);
    let max_cols = if max_cols == usize::MAX {
        default_max
    } else {
        max_cols.clamp(1, 48)
    };
    terminal_image::render_inline_image_with_max_cols(&png, max_cols)
}

/// 渲染文字与公式混合的表格单元格 PNG。
///
/// 参数:
/// - `source`: 含公式标记的单元格内容
/// - `temp_dir`: 临时输出目录
///
/// 返回:
/// - PNG 文件路径
fn render_mixed_image(source: &str, temp_dir: &TempDir) -> Result<PathBuf> {
    if let Some(output) = try_render_typst_mixed(source, temp_dir)? {
        return Ok(output);
    }
    let latex = mixed_to_latex(source);
    if let Some(output) = try_render_ratex(&latex, temp_dir, MathRenderMode::Inline)? {
        return Ok(output);
    }
    let svg = temp_dir.path().join("formula.svg");
    let output = temp_dir.path().join("formula.png");
    fs::write(&svg, build_fallback_svg(source, MathRenderMode::Inline))
        .with_context(|| format!("failed to write {}", svg.display()))?;
    convert_svg_to_png(&svg, &output)?;
    ensure_file_exists(&output)?;
    Ok(output)
}

/// 将混合单元格内容转换为 LaTeX 源码。
///
/// 参数:
/// - `source`: 原始单元格内容
///
/// 返回:
/// - 文字与公式拼接后的 LaTeX
fn mixed_to_latex(source: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '$' && chars[index + 1] == '$' {
            if let Some(end) = find_math_end(&chars, index + 2, true) {
                output.extend(chars[index + 2..end].iter());
                index = end + 2;
                continue;
            }
        }
        if chars[index] == '$' {
            if let Some(end) = find_math_end(&chars, index + 1, false) {
                output.extend(chars[index + 1..end].iter());
                index = end + 1;
                continue;
            }
        }
        let start = index;
        while index < chars.len() && chars[index] != '$' {
            index += 1;
        }
        let text = chars[start..index].iter().collect::<String>();
        if !text.is_empty() {
            let escaped = text
                .replace('\\', "\\textbackslash{}")
                .replace('{', "\\{")
                .replace('}', "\\}");
            output.push_str("\\text{");
            output.push_str(&escaped);
            output.push('}');
        }
    }
    output
}

/// 查找行内或显示公式的结束标记。
///
/// 参数:
/// - `chars`: 原始字符
/// - `start`: 内容起始位置
/// - `display`: 是否使用双美元标记
///
/// 返回:
/// - 结束标记起始位置
fn find_math_end(chars: &[char], start: usize, display: bool) -> Option<usize> {
    let mut index = start;
    while index < chars.len() {
        if display {
            if chars[index] == '$' && chars.get(index + 1) == Some(&'$') {
                return Some(index);
            }
        } else if chars[index] == '$' {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// 尝试使用 Typst 渲染混合单元格。
///
/// 参数:
/// - `source`: 原始单元格内容
/// - `temp_dir`: 临时输出目录
///
/// 返回:
/// - 成功时返回 PNG 路径
fn try_render_typst_mixed(source: &str, temp_dir: &TempDir) -> Result<Option<PathBuf>> {
    if !command_available("typst") {
        return Ok(None);
    }
    let input = temp_dir.path().join("mixed_cell.typ");
    let output = temp_dir.path().join("mixed_cell.png");
    let body = escape_typst_mixed(source);
    let content = format!(
        "#set page(width: auto, height: auto, margin: 2pt)\n#set text(fill: rgb(\"d7e3ff\"), size: 11pt)\n{body}\n"
    );
    fs::write(&input, content).with_context(|| format!("failed to write {}", input.display()))?;
    let mut command = Command::new("typst");
    command
        .arg("compile")
        .arg(&input)
        .arg(&output)
        .stdin(Stdio::null());
    if run_command(command, "typst").is_ok() && output.is_file() {
        return Ok(Some(output));
    }
    Ok(None)
}

/// 转义 Typst 混合单元格，保留公式标记。
///
/// 参数:
/// - `source`: 原始单元格内容
///
/// 返回:
/// - Typst 正文
fn escape_typst_mixed(source: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;
    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '$' && chars[index + 1] == '$' {
            if let Some(end) = find_math_end(&chars, index + 2, true) {
                output.push_str("$ ");
                output.extend(chars[index + 2..end].iter());
                output.push_str(" $");
                index = end + 2;
                continue;
            }
        }
        if chars[index] == '$' {
            if let Some(end) = find_math_end(&chars, index + 1, false) {
                output.push('$');
                output.extend(chars[index + 1..end].iter());
                output.push('$');
                index = end + 1;
                continue;
            }
        }
        match chars[index] {
            '#' => output.push_str("\\#"),
            '\\' => output.push_str("\\\\"),
            '*' => output.push_str("\\*"),
            '_' => output.push_str("\\_"),
            '`' => output.push_str("\\`"),
            '<' => output.push_str("\\<"),
            '>' => output.push_str("\\>"),
            '@' => output.push_str("\\@"),
            other => output.push(other),
        }
        index += 1;
    }
    output
}

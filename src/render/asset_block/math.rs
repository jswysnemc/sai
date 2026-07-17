use super::commands::{command_available, ensure_file_exists, run_command};
use super::{render_error, render_success, test_stub_enabled, MathRenderMode};
use crate::render::terminal_image;
use anyhow::{Context, Result};
use ratex_layout::{layout, to_display_list, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// 渲染数学公式源码。
///
/// 参数:
/// - `source`: 数学公式源码
/// - `mode`: 块级或行内展示模式
///
/// 返回:
/// - 终端图片协议文本或错误提示
pub(super) fn render_source(source: &str, mode: MathRenderMode) -> String {
    if source.trim().is_empty() {
        return render_error("math", "content is empty");
    }
    if test_stub_enabled() {
        let placeholder = match mode {
            MathRenderMode::Block => "[asset rendering skipped]\n".to_string(),
            MathRenderMode::Inline => "[inline math rendering skipped]\n".to_string(),
        };
        return match mode {
            MathRenderMode::Block => render_success(placeholder),
            MathRenderMode::Inline => placeholder,
        };
    }
    match render_terminal(source, mode) {
        Ok(rendered) => match mode {
            MathRenderMode::Block => render_success(rendered),
            MathRenderMode::Inline => rendered,
        },
        Err(error) => render_error("math", &error.to_string()),
    }
}

/// 生成公式图片并转换为终端图片协议。
///
/// 参数:
/// - `source`: 数学公式源码
/// - `mode`: 展示模式
///
/// 返回:
/// - 终端图片协议文本
fn render_terminal(source: &str, mode: MathRenderMode) -> Result<String> {
    let temp_dir = tempfile::tempdir().context("failed to create temporary render directory")?;
    let image = render_image(source, &temp_dir, mode)?;
    terminal_image::render_terminal_image(&image)
}

/// 按降级顺序生成数学公式 PNG。
///
/// 参数:
/// - `source`: 数学公式源码
/// - `temp_dir`: 临时输出目录
/// - `mode`: 展示模式
///
/// 返回:
/// - PNG 文件路径
pub(super) fn render_image(
    source: &str,
    temp_dir: &TempDir,
    mode: MathRenderMode,
) -> Result<PathBuf> {
    if let Some(output) = try_render_ratex(source, temp_dir, mode)? {
        return Ok(output);
    }
    if let Some(output) = try_render_typst(source, temp_dir, mode)? {
        return Ok(output);
    }
    let svg = temp_dir.path().join("formula.svg");
    let output = temp_dir.path().join("formula.png");
    fs::write(&svg, build_fallback_svg(source, mode))
        .with_context(|| format!("failed to write {}", svg.display()))?;
    convert_svg_to_png(&svg, &output)?;
    ensure_file_exists(&output)?;
    Ok(output)
}

/// 使用 RaTeX 纯 Rust 管线渲染公式。
///
/// 参数:
/// - `source`: 数学公式源码
/// - `temp_dir`: 临时输出目录
/// - `mode`: 展示模式
///
/// 返回:
/// - 成功时返回 PNG 路径，解析失败时返回空
pub(super) fn try_render_ratex(
    source: &str,
    temp_dir: &TempDir,
    mode: MathRenderMode,
) -> Result<Option<PathBuf>> {
    let formula = normalize_source(source);
    let ast = match parse(&formula) {
        Ok(ast) => ast,
        Err(_) => return Ok(None),
    };
    let color = Color::parse("#d7e3ff").unwrap_or(Color::BLACK);
    let math_style = match mode {
        MathRenderMode::Block => MathStyle::Display,
        MathRenderMode::Inline => MathStyle::Text,
    };
    let (font_size, padding, device_pixel_ratio) = match mode {
        MathRenderMode::Block => (28.0, 4.0, 1.5),
        MathRenderMode::Inline => (18.0, 0.0, 1.5),
    };
    let layout_opts = LayoutOptions::default()
        .with_style(math_style)
        .with_color(color);
    let render_opts = RenderOptions {
        font_size,
        padding,
        background_color: transparent_color(),
        font_dir: String::new(),
        device_pixel_ratio,
    };
    let layout_box = layout(&ast, &layout_opts);
    let display_list = to_display_list(&layout_box);
    let png = match render_to_png(&display_list, &render_opts) {
        Ok(png) => png,
        Err(_) => return Ok(None),
    };
    let output = temp_dir.path().join("formula.png");
    fs::write(&output, png).with_context(|| format!("failed to write {}", output.display()))?;
    ensure_file_exists(&output)?;
    Ok(Some(output))
}

/// 构造透明背景颜色。
///
/// 返回:
/// - 全透明颜色
fn transparent_color() -> Color {
    Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    }
}

/// 归一化数学公式源码。
///
/// 参数:
/// - `source`: 原始公式
///
/// 返回:
/// - 适合解析器处理的单行公式
fn normalize_source(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 尝试使用 Typst 渲染数学公式。
///
/// 参数:
/// - `source`: 数学公式源码
/// - `temp_dir`: 临时输出目录
/// - `mode`: 展示模式
///
/// 返回:
/// - Typst 可用且成功时返回 PNG 路径
fn try_render_typst(
    source: &str,
    temp_dir: &TempDir,
    mode: MathRenderMode,
) -> Result<Option<PathBuf>> {
    if !command_available("typst") {
        return Ok(None);
    }
    let input = temp_dir.path().join("formula.typ");
    let output = temp_dir.path().join("formula.png");
    let (margin, text_size) = match mode {
        MathRenderMode::Block => (6, 14),
        MathRenderMode::Inline => (2, 11),
    };
    let content = format!(
        "#set page(width: auto, height: auto, margin: {margin}pt)\n#set text(fill: rgb(\"d7e3ff\"), size: {text_size}pt)\n$ {} $\n",
        source.trim()
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

/// 将 SVG 公式图片转换为 PNG。
///
/// 参数:
/// - `svg`: SVG 输入路径
/// - `output`: PNG 输出路径
///
/// 返回:
/// - 转换是否成功
pub(super) fn convert_svg_to_png(svg: &Path, output: &Path) -> Result<()> {
    let mut rsvg = Command::new("rsvg-convert");
    rsvg.arg("-f")
        .arg("png")
        .arg("-o")
        .arg(output)
        .arg(svg)
        .stdin(Stdio::null());
    if run_command(rsvg, "rsvg-convert").is_ok() {
        return Ok(());
    }
    let mut magick = Command::new("magick");
    magick.arg(svg).arg(output).stdin(Stdio::null());
    run_command(magick, "magick")
}

/// 构建数学公式降级 SVG。
///
/// 参数:
/// - `source`: 数学公式源码
/// - `mode`: 展示模式
///
/// 返回:
/// - SVG 文本
pub(super) fn build_fallback_svg(source: &str, mode: MathRenderMode) -> String {
    let lines = wrap_lines(source.trim(), 72);
    let (font_size, char_width, line_height, side_padding, top_padding) = match mode {
        MathRenderMode::Block => (17, 9, 24, 28, 30),
        MathRenderMode::Inline => (13, 7, 18, 12, 20),
    };
    let max_chars = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(1);
    let min_width = match mode {
        MathRenderMode::Block => 180,
        MathRenderMode::Inline => 40,
    };
    let min_height = match mode {
        MathRenderMode::Block => 48,
        MathRenderMode::Inline => 24,
    };
    let width = (max_chars * char_width + side_padding * 2).clamp(min_width, 1200);
    let height = (lines.len() * line_height + top_padding).clamp(min_height, 1200);
    let mut text = String::new();
    for (index, line) in lines.iter().enumerate() {
        let y = top_padding + index * line_height;
        text.push_str(&format!(
            r##"<text x="{side_padding}" y="{y}" font-family="DejaVu Sans Mono, monospace" font-size="{font_size}" fill="#d7e3ff">{}</text>"##,
            escape_xml(line)
        ));
    }
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">{text}</svg>"##
    )
}

/// 按字符数量拆分降级公式行。
///
/// 参数:
/// - `source`: 原始公式
/// - `limit`: 单行最大字符数
///
/// 返回:
/// - 拆分后的行
fn wrap_lines(source: &str, limit: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw_line in source.lines() {
        let mut current = String::new();
        for ch in raw_line.chars() {
            current.push(ch);
            if current.chars().count() >= limit {
                lines.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(source.to_string());
    }
    lines
}

/// 转义 XML 文本。
///
/// 参数:
/// - `text`: 原始文本
///
/// 返回:
/// - XML 安全文本
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

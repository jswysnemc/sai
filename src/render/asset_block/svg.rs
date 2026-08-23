use super::commands::ensure_file_exists;
use crate::render::terminal_image;
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use tempfile::TempDir;

/// 终端图片允许的最长边像素，避免超大 SVG 撑爆内存。
const MAX_EDGE_PX: f32 = 2048.0;

/// 将 SVG 源码生成 PNG 并转换为终端图片协议。
///
/// 参数:
/// - `source`: SVG 源码
///
/// 返回:
/// - 当前终端支持的图片协议文本
pub(super) fn render_terminal(source: &str) -> Result<String> {
    let temp_dir = tempfile::tempdir().context("failed to create temporary render directory")?;
    let image = render_image(source, &temp_dir)?;
    terminal_image::render_terminal_image(&image)
}

/// 使用 resvg 将独立 SVG 文档光栅化为透明 PNG。
///
/// 参数:
/// - `source`: SVG 源码
/// - `temp_dir`: 临时输出目录
///
/// 返回:
/// - PNG 文件路径
pub(super) fn render_image(source: &str, temp_dir: &TempDir) -> Result<PathBuf> {
    if !is_svg_markup(source) {
        bail!("content is not a standalone SVG document")
    }
    let output = temp_dir.path().join("diagram.png");
    rasterize_svg(source.trim(), &output)?;
    ensure_file_exists(&output)?;
    Ok(output)
}

/// 判断文本是否为独立 SVG 文档。
///
/// 参数:
/// - `source`: 待检查文本
///
/// 返回:
/// - 去除空白后仅为 SVG 根元素时为 true
pub(crate) fn is_svg_markup(source: &str) -> bool {
    let rest = skip_xml_declaration(source.trim()).trim_start();
    starts_with_svg_tag(rest) && ends_with_svg_close(rest)
}

/// 判断一行是否像 SVG 块的起始。
///
/// 参数:
/// - `line`: Markdown 原始行
///
/// 返回:
/// - 该行以 `<svg` 或同段 XML 声明后的 `<svg` 开头时为 true
pub(crate) fn looks_like_svg_start(line: &str) -> bool {
    starts_with_svg_tag(skip_xml_declaration(line.trim_start()).trim_start())
}

/// 判断一行是否包含 SVG 结束标签。
///
/// 参数:
/// - `line`: Markdown 原始行
///
/// 返回:
/// - 包含 `</svg>` 时为 true
pub(crate) fn contains_svg_close(line: &str) -> bool {
    line.to_ascii_lowercase().contains("</svg>")
}

/// 把 SVG 源码光栅化到指定 PNG 路径。
///
/// 参数:
/// - `source`: 已确认的 SVG 源码
/// - `output`: PNG 输出路径
///
/// 返回:
/// - 写入是否成功
fn rasterize_svg(source: &str, output: &std::path::Path) -> Result<()> {
    // 1. 解析 SVG，缺省尺寸与 mermaid 管线保持一致
    let mut options = usvg::Options {
        default_size: usvg::Size::from_wh(800.0, 600.0)
            .unwrap_or_else(|| usvg::Size::from_wh(1.0, 1.0).expect("nonzero default size")),
        ..usvg::Options::default()
    };
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(source, &options).context("failed to parse svg")?;

    // 2. 按最长边夹取画布，保持宽高比
    let size = tree.size();
    let width = size.width().max(1.0);
    let height = size.height().max(1.0);
    let scale = (MAX_EDGE_PX / width).min(MAX_EDGE_PX / height).min(1.0);
    let pixel_width = (width * scale).round().max(1.0) as u32;
    let pixel_height = (height * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(pixel_width, pixel_height)
        .context("failed to allocate svg pixmap")?;

    // 3. 透明底渲染后写 PNG
    let transform = resvg::tiny_skia::Transform::from_scale(
        pixel_width as f32 / width,
        pixel_height as f32 / height,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap
        .save_png(output)
        .with_context(|| format!("failed to write {}", output.display()))
}

/// 去掉可选的 XML 声明，返回后续文本。
///
/// 参数:
/// - `source`: 原始文本
///
/// 返回:
/// - 声明之后的切片；没有声明时返回原文
fn skip_xml_declaration(source: &str) -> &str {
    let trimmed = source.trim_start();
    if !trimmed.get(..5).is_some_and(|prefix| prefix.eq_ignore_ascii_case("<?xml")) {
        return source;
    }
    match trimmed.find("?>") {
        Some(index) => &trimmed[index + 2..],
        None => source,
    }
}

/// 判断文本是否以 SVG 开始标签开头。
///
/// 参数:
/// - `source`: 已去除前置空白的文本
///
/// 返回:
/// - 以 `<svg` 且后接空白、`>` 或 `/` 时为 true
fn starts_with_svg_tag(source: &str) -> bool {
    let Some(rest) = source.get(4..) else {
        return false;
    };
    source.get(..4).is_some_and(|prefix| prefix.eq_ignore_ascii_case("<svg"))
        && rest
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/'))
}

/// 判断文本是否以 SVG 结束标签收尾。
///
/// 参数:
/// - `source`: 待检查文本
///
/// 返回:
/// - 去除尾部空白后以 `</svg>` 结尾时为 true
fn ends_with_svg_close(source: &str) -> bool {
    source.trim_end().to_ascii_lowercase().ends_with("</svg>")
}

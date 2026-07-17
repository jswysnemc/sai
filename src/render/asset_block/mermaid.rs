use super::commands::ensure_file_exists;
use crate::render::terminal_image;
use anyhow::{Context, Result};
use std::path::PathBuf;
use tempfile::TempDir;

/// 将 Mermaid 源码生成 PNG 并转换为终端图片协议。
///
/// 参数:
/// - `source`: Mermaid 源码
///
/// 返回:
/// - 当前终端支持的图片协议文本
pub(super) fn render_terminal(source: &str) -> Result<String> {
    let temp_dir = tempfile::tempdir().context("failed to create temporary render directory")?;
    let image = render_image(source, &temp_dir)?;
    terminal_image::render_terminal_image(&image)
}

/// 使用纯 Rust Mermaid 渲染器生成透明 PNG。
///
/// 参数:
/// - `source`: Mermaid 源码
/// - `temp_dir`: 临时输出目录
///
/// 返回:
/// - PNG 文件路径
pub(super) fn render_image(source: &str, temp_dir: &TempDir) -> Result<PathBuf> {
    let output = temp_dir.path().join("diagram.png");
    let theme = transparent_theme();
    let svg = mermaid_rs_renderer::render_with_options(
        source,
        mermaid_rs_renderer::RenderOptions {
            theme: theme.clone(),
            layout: mermaid_rs_renderer::LayoutConfig::default(),
        },
    )
    .context("failed to render mermaid svg")?;
    mermaid_rs_renderer::write_output_png(
        &svg,
        &output,
        &mermaid_rs_renderer::RenderConfig::default(),
        &theme,
    )
    .with_context(|| format!("failed to write {}", output.display()))?;
    ensure_file_exists(&output)?;
    Ok(output)
}

/// 构造透明画布的 Mermaid 主题。
///
/// 返回:
/// - Mermaid 渲染主题
fn transparent_theme() -> mermaid_rs_renderer::Theme {
    let mut theme = mermaid_rs_renderer::Theme::modern();
    theme.background = "transparent".to_string();
    theme
}

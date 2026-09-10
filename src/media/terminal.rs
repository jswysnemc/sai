use crate::render::terminal_image;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::Path;

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;

/// 【终端图片】【渲染】只生成终端协议内容，不写入输出流。
/// @param path 图片路径；size 为可选单元格尺寸
/// @returns 可直接显示的协议内容
pub(crate) fn render(path: &Path, size: Option<&str>) -> Result<String> {
    terminal_image::render_buffered_image(path, size)
        .with_context(|| format!("failed to render image {}", path.display()))
}

/// 【终端图片】【输出】完整渲染成功后一次提交，维持图片前后的空行。
/// @param rendered 已完成的协议内容
/// @returns 标准输出写入结果
pub(crate) fn print_rendered(rendered: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output)?;
    write!(output, "{rendered}")?;
    if !rendered.ends_with('\n') {
        writeln!(output)?;
    }
    writeln!(output)?;
    output.flush()?;
    Ok(())
}

/// 【终端图片】【共享绘制】为表情库提供公共终端绘制能力。
/// @param path 图片路径；size 为可选单元格尺寸
/// @returns 完成渲染和输出时成功
pub(crate) async fn print_image_file(path: &Path, size: Option<String>) -> Result<()> {
    print_rendered(&render(path, size.as_deref())?)
}

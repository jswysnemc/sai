use super::{
    kitty_block_limits, kitty_cell_dimensions_with, kitty_image_id, kitty_placement_payload,
    kitty_transmission_payload, next_kitty_placement_id, render_iterm_image_with_cells,
    render_kitty_image_with, render_terminal_image_with_size, supports_iterm_inline_image,
    supports_kitty_graphics, TerminalImageSize,
};
use anyhow::{Context, Result};
use std::path::Path;

/// 【终端图片】【缓冲渲染】完整生成图片协议，不写入终端或消耗传输缓存。
/// @param path 图片路径；size 为可选单元格上限
/// @returns 可以丢弃或一次提交的完整图片内容
pub(crate) fn render_buffered_image(path: &Path, size: Option<&str>) -> Result<String> {
    let requested = TerminalImageSize::parse(size);
    let limits = bounded_limits(&requested);
    if supports_kitty_graphics() {
        return render_kitty_image_with(path, limits, encode_kitty_buffered);
    }
    if supports_iterm_inline_image() {
        let (width, height) = image::ImageReader::open(path)?
            .with_guessed_format()?
            .into_dimensions()?;
        let (columns, rows) = kitty_cell_dimensions_with(limits, width as usize, height as usize);
        return render_iterm_image_with_cells(path, columns, rows);
    }
    render_terminal_image_with_size(path, size)
}

/// 【终端图片】【尺寸约束】取当前终端、调用方尺寸与硬上限的交集。
/// @param requested 已解析的单元格尺寸
/// @returns 最大列行与每个单元格的像素尺寸
fn bounded_limits(requested: &TerminalImageSize) -> (usize, usize, usize, usize) {
    let (columns, rows, pixel_width, pixel_height) = kitty_block_limits();
    (
        columns.min(requested.width_cells.unwrap_or(300)).min(300),
        rows.min(requested.height_cells.unwrap_or(200)).min(200),
        pixel_width,
        pixel_height,
    )
}

/// 【终端图片】【Kitty 缓冲】传输与放置都放入返回值，丢弃结果不会影响下一次显示。
/// @param path 已处理的 PNG；columns、rows 为实际单元格尺寸
/// @returns 完整传输和放置协议，不改变全局传输记录
fn encode_kitty_buffered(
    path: &Path,
    columns: Option<usize>,
    rows: Option<usize>,
) -> Result<String> {
    let bytes = std::fs::read(path).context("read buffered terminal image")?;
    let image_id = kitty_image_id(&bytes);
    let mut output = kitty_transmission_payload(image_id, &bytes);
    output.push_str(&kitty_placement_payload(
        image_id,
        next_kitty_placement_id(),
        columns,
        rows,
    ));
    Ok(output)
}

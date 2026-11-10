//! 居中图片浮窗的布局与绘制，图片和关闭控件使用同一组几何数据。

use super::{
    image_viewer::ImageViewer, image_viewer_layout::ViewerLayout, image_window,
    layout::FullscreenLayout,
};
use crate::render::{
    ansi_clip, ansi_overlay::overlay_columns, image_reference, terminal_text as t,
};

const BORDER: &str = "\x1b[38;5;244m";
const RESET: &str = "\x1b[0m";

/// 【图片浮窗】【窗口绘制】在原画面局部叠加等比大图，保留浮窗外的正文与图片。
/// 参数: viewer 为图片状态，layout 为分区，rows 为屏幕行；返回: 无。
pub(super) fn overlay(viewer: &mut ImageViewer, layout: &FullscreenLayout, rows: &mut [String]) {
    viewer.image_area = None;
    let Some(source) = viewer.source.as_deref() else {
        return;
    };
    let geometry = ViewerLayout::compute(layout);
    // 1. 【图片浮窗】【遮挡隔离】仅撤下与浮窗相交的放置，不能清空整片背景
    image_window::mask_placements(
        rows,
        geometry.left,
        geometry.top,
        geometry.width,
        geometry.height,
    );
    let name = if source.starts_with("data:") {
        t("Attached image", "图片附件").to_string()
    } else {
        std::path::Path::new(source)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    let name = ansi_clip::strip_ansi(&name)
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>();
    let title = format!("{} · {name}", t("Image", "图片"));
    let title = ansi_clip::clip_ansi_to_width(&title, geometry.width.saturating_sub(7), "");
    let gap = geometry
        .width
        .saturating_sub(unicode_width::UnicodeWidthStr::width(title.as_str()) + 6);
    let close = if viewer.close_hovered {
        "\x1b[7m × \x1b[27m"
    } else {
        " × "
    };
    put(
        rows,
        geometry.top,
        geometry,
        &format!(
            "{BORDER}╭ {RESET}{title}{BORDER}{}{close}╮{RESET}",
            "─".repeat(gap)
        ),
    );
    if geometry.height < 3 {
        return;
    }
    let inner_width = geometry.width.saturating_sub(4).max(1);
    let inner_height = geometry.height - 2;
    let preview = crate::render::image_preview::render_large(source, inner_width, inner_height);
    let available = preview.is_ok();
    let image = preview.unwrap_or_else(|_| {
        format!(
            "\x1b[2m{}{RESET}",
            t("Image preview unavailable.", "图片预览不可用。")
        )
    });
    let (image_width, image_height) = image_reference::dimensions(&image);
    let area = geometry.image_area(image_width, image_height);
    viewer.image_area = available.then_some(area);
    let indent = area.left - geometry.left - 1;
    let offset = area.top - geometry.top - 1;
    let lines = image.split('\n').collect::<Vec<_>>();
    // 2. 【图片浮窗】【比例布局】协议声明占位和字符图像都按实际尺寸居中，不拉伸图片
    for row in 0..inner_height {
        let content = row
            .checked_sub(offset)
            .and_then(|row| lines.get(row))
            .copied()
            .unwrap_or("");
        let content = ansi_clip::clip_ansi_to_width(content, inner_width, "");
        let visible =
            unicode_width::UnicodeWidthStr::width(ansi_clip::strip_ansi(&content).as_str());
        let right = geometry.width.saturating_sub(indent + visible + 2);
        put(
            rows,
            geometry.top + row + 1,
            geometry,
            &format!(
                "{BORDER}│{RESET}{}{content}{RESET}{}{BORDER}│{RESET}",
                " ".repeat(indent),
                " ".repeat(right)
            ),
        );
    }
    let hint = t("Esc / click outside to close", "Esc / 点击图片外关闭");
    let hint_width = unicode_width::UnicodeWidthStr::width(hint);
    let bottom = if geometry.width >= hint_width + 6 {
        format!("─ {hint} {}", "─".repeat(geometry.width - hint_width - 5))
    } else {
        "─".repeat(geometry.width.saturating_sub(2))
    };
    put(
        rows,
        geometry.top + geometry.height - 1,
        geometry,
        &format!("{BORDER}╰{bottom}╯{RESET}"),
    );
}

/// 【图片浮窗】【行绘制】只替换浮窗列区间，两侧恢复原有文字、颜色和链接。
/// 参数: rows 为屏幕行，row 为行号，geometry 为浮窗布局，text 为内容；返回: 无。
fn put(rows: &mut [String], row: usize, geometry: ViewerLayout, text: &str) {
    if let Some(line) = rows.get_mut(row) {
        let text = ansi_clip::clip_ansi_to_width(text, geometry.width, "");
        *line = overlay_columns(line, geometry.left, geometry.width, &text);
    }
}

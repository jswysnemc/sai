//! 居中图片浮窗的布局与绘制，图片和关闭控件使用同一组几何数据。

use super::{image_viewer::ImageViewer, layout::FullscreenLayout};
use crate::render::{ansi_clip, image_reference, terminal_text as t};

const BORDER: &str = "\x1b[38;5;244m";
const RESET: &str = "\x1b[0m";

#[derive(Clone, Copy, Debug)]
pub(super) struct ViewerLayout {
    pub(super) left: usize,
    pub(super) top: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl ViewerLayout {
    /// 【图片浮窗】【自适应布局】使用正文区，保留原来的顶部导航与底部输入区。
    /// 参数: layout 为主界面分区；返回: 浮窗边界，尺寸不足时使用紧凑布局。
    pub(super) fn compute(layout: &FullscreenLayout) -> Self {
        let columns = usize::from(layout.cols);
        let rows = usize::from(layout.body_height);
        let width = columns
            .saturating_sub(if columns >= 16 { 4 } else { 0 })
            .min(124);
        let height = rows.saturating_sub(if rows >= 8 { 2 } else { 0 });
        Self {
            left: (columns - width) / 2,
            top: usize::from(layout.body_top) + (rows - height) / 2,
            width,
            height,
        }
    }

    /// 【图片浮窗】【关闭命中】右上角三列按钮与显示范围完全一致。
    /// 参数: column/row 为鼠标坐标；返回: 是否命中关闭按钮。
    pub(super) fn close_at(self, column: u16, row: u16) -> bool {
        usize::from(row) == self.top
            && usize::from(column) >= self.left + self.width.saturating_sub(4)
            && usize::from(column) < self.left + self.width.saturating_sub(1)
    }
}

/// 【图片浮窗】【窗口绘制】清除正文中的底层图片，居中显示等比大图和紧凑标题。
/// 参数: viewer 为图片状态，layout 为分区，rows 为屏幕行；返回: 无。
pub(super) fn overlay(viewer: &ImageViewer, layout: &FullscreenLayout, rows: &mut [String]) {
    let Some(source) = viewer.source.as_deref() else {
        return;
    };
    let geometry = ViewerLayout::compute(layout);
    let columns = usize::from(layout.cols);
    // 1. 【图片浮窗】【遮挡隔离】不能只覆盖字符，否则底层 Kitty 放置仍会越过浮窗边框
    for row in rows
        .iter_mut()
        .skip(usize::from(layout.body_top))
        .take(usize::from(layout.body_height))
    {
        *row = " ".repeat(columns);
    }
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
        geometry.left,
        columns,
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
    let image = crate::render::image_preview::render_large(source, inner_width, inner_height)
        .unwrap_or_else(|_| {
            format!(
                "\x1b[2m{}{RESET}",
                t("Image preview unavailable.", "图片预览不可用。")
            )
        });
    let (image_width, image_height) = image_reference::dimensions(&image);
    let indent = (geometry.width.saturating_sub(image_width + 2)) / 2;
    let offset = inner_height.saturating_sub(image_height) / 2;
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
            geometry.left,
            columns,
            &format!(
                "{BORDER}│{RESET}{}{content}{RESET}{}{BORDER}│{RESET}",
                " ".repeat(indent),
                " ".repeat(right)
            ),
        );
    }
    put(
        rows,
        geometry.top + geometry.height - 1,
        geometry.left,
        columns,
        &format!(
            "{BORDER}╰{}╯{RESET}",
            "─".repeat(geometry.width.saturating_sub(2))
        ),
    );
}

/// 【图片浮窗】【行绘制】写入定宽浮窗行，保持两侧留白和默认背景。
/// 参数: rows 为屏幕行，row/left 为位置，columns 为终端宽度，text 为内容；返回: 无。
fn put(rows: &mut [String], row: usize, left: usize, columns: usize, text: &str) {
    if let Some(line) = rows.get_mut(row) {
        let text = ansi_clip::clip_ansi_to_width(text, columns.saturating_sub(left), "");
        let width = unicode_width::UnicodeWidthStr::width(ansi_clip::strip_ansi(&text).as_str());
        *line = format!(
            "{}{text}{RESET}{}",
            " ".repeat(left),
            " ".repeat(columns.saturating_sub(left + width))
        );
    }
}

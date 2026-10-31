//! 图片点击元数据在排版时转换为行侧数据，不作为终端控制协议发送。

use base64::{engine::general_purpose::STANDARD, Engine};
use std::sync::Arc;

const PREFIX: &str = "\x1b]sai-image;";

/// 【终端图片】【排版识别】识别需要跨物理行保留图片元数据的文本块。
/// 参数: text 为排版输入；返回: 是否含有内部图片标记。
pub(crate) fn has_marker(text: &str) -> bool {
    text.contains(PREFIX)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageLine {
    pub(crate) source: Arc<str>,
    pub(crate) column: usize,
    pub(crate) width: usize,
}

/// 【终端图片】【尺寸读取】读取协议声明尺寸或字符图像的实际尺寸。
/// 参数: rendered 为完整图片文本；返回: 图片占用的列数和行数。
pub(crate) fn dimensions(rendered: &str) -> (usize, usize) {
    let rows = super::terminal_image::reserved_image_rows(rendered)
        .unwrap_or_else(|| rendered.trim_end_matches('\n').split('\n').count());
    let columns = rendered
        .split("\x1b_Ga=p")
        .nth(1)
        .and_then(|part| part.split("\x1b\\").next())
        .and_then(|part| part.split(",c=").nth(1))
        .and_then(|part| part.split(',').next())
        .and_then(|part| part.parse().ok())
        .or_else(|| super::terminal_image::sixel_image_cells(rendered).map(|(cols, _)| cols))
        .or_else(|| {
            rendered
                .split("\x1b]1337;File=")
                .nth(1)
                .and_then(|part| part.split("width=").nth(1))
                .and_then(|part| part.split(';').next())
                .and_then(|part| part.parse().ok())
        })
        .unwrap_or_else(|| {
            rendered
                .split('\n')
                .map(|line| {
                    unicode_width::UnicodeWidthStr::width(
                        super::ansi_clip::strip_ansi(line).as_str(),
                    )
                })
                .max()
                .unwrap_or(1)
        });
    (columns.max(1), rows.max(1))
}

/// 【终端图片】【来源标记】为缓存文本附加零宽元数据，由正文排版入口消费。
/// 参数: source 为路径或数据 URL，rendered 为缩略图；返回: 带内部标记的排版输入。
pub(crate) fn annotate(source: &str, rendered: &str) -> String {
    let (columns, rows) = dimensions(rendered);
    format!(
        "{PREFIX}{columns};{rows};{}\x1b\\{rendered}",
        STANDARD.encode(source)
    )
}

/// 【终端图片】【元数据提取】移除内部标记并记录前导缩进，避免协议或图片来源泄漏。
/// 参数: text 为单个物理行；返回: 终端文本及可选图片行元数据和占位高度。
pub(crate) fn extract(text: &str) -> (String, Option<(ImageLine, usize)>) {
    let Some(start) = text.find(PREFIX) else {
        return (text.to_string(), None);
    };
    let Some(end) = text[start..].find("\x1b\\").map(|end| start + end) else {
        return (text[..start].to_string(), None);
    };
    let mut fields = text[start + PREFIX.len()..end].splitn(3, ';');
    let metadata = (|| {
        let width = fields.next()?.parse::<usize>().ok()?.clamp(1, 300);
        let rows = fields.next()?.parse::<usize>().ok()?.clamp(1, 200);
        let source = String::from_utf8(STANDARD.decode(fields.next()?).ok()?).ok()?;
        let column = unicode_width::UnicodeWidthStr::width(
            super::ansi_clip::strip_ansi(&text[..start]).as_str(),
        );
        Some((
            ImageLine {
                source: Arc::from(source),
                column,
                width,
            },
            rows,
        ))
    })();
    (format!("{}{}", &text[..start], &text[end + 2..]), metadata)
}

#[cfg(test)]
#[path = "image_reference_tests.rs"]
mod tests;

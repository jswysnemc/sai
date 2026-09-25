use super::image_resize::{image_metadata_text, process_image};
use super::limits::max_output_tokens;
use crate::tools::fs_path::fs_error;
use crate::tools::{ToolModelAttachment, ToolOutput};
use anyhow::{bail, Result};
use std::path::Path;

/// 按图片处理的扩展名。
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// 判断扩展名是否按图片读取。
///
/// 参数:
/// - `extension`: 小写扩展名
///
/// 返回:
/// - 属于图片扩展名时为 true
pub(super) fn is_image_extension(extension: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&extension)
}

/// 读取本地图片，处理到 token 预算内后作为模型附件返回。
///
/// 文本结果只写来源、格式、大小与尺寸；图片本体通过附件直接交给当前模型，
/// 不经过任何文字描述回退。
///
/// 参数:
/// - `path`: 图片路径
///
/// 返回:
/// - 带图片附件的工具结果
pub(super) fn read_image(path: &Path) -> Result<ToolOutput> {
    let bytes = std::fs::read(path).map_err(|error| fs_error("read image", path, &error))?;
    if bytes.is_empty() {
        bail!("Image file is empty: {}", path.display())
    }
    let processed = process_image(&bytes, max_output_tokens())?;
    let source = path.display().to_string();
    let note = image_metadata_text(&source, &processed, bytes.len() as u64);
    let attachment = ToolModelAttachment::new(processed.data_url(), source, note.clone());
    Ok(ToolOutput::text(note).with_model_attachments([attachment]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    /// 图片读取只在文本里写元信息，图片数据只出现在附件中。
    #[test]
    fn image_read_returns_attachment_without_base64_text() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("shot.png");
        let rgba = image::RgbaImage::from_pixel(8, 4, image::Rgba([200, 10, 10, 255]));
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(rgba.as_raw(), 8, 4, image::ExtendedColorType::Rgba8)
            .unwrap();
        std::fs::write(&path, bytes).unwrap();

        let output = read_image(&path).unwrap();

        assert!(
            output.content.starts_with("[Image: source: "),
            "{}",
            output.content
        );
        assert!(output.content.contains("8x4"), "{}", output.content);
        assert!(!output.content.contains("base64"));
        assert_eq!(output.model_attachments.len(), 1);
        assert!(output.model_attachments[0]
            .image_url
            .starts_with("data:image/png;base64,"));
    }
}

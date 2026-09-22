use super::response::ImagePayload;
use anyhow::{bail, Context, Result};
use image::ImageFormat;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_IMAGES_PER_REQUEST: usize = 8;

/// 一张已写入本地缓存的图片。
#[derive(Debug, Clone)]
pub(super) struct StoredImage {
    pub(super) path: PathBuf,
    pub(super) file_name: String,
    pub(super) mime: String,
    pub(super) bytes: usize,
}

/// 将供应商图片写入受控缓存目录并推导稳定媒体类型。
///
/// 参数:
/// - root: 生图缓存根目录
/// - payload: 已解码图片
/// - index: 当前请求中的图片序号
///
/// 返回:
/// - 本地图片路径、媒体文件名和 MIME 类型
pub(super) fn store(root: &Path, payload: &ImagePayload, index: usize) -> Result<StoredImage> {
    if index >= MAX_IMAGES_PER_REQUEST {
        bail!("image endpoint returned too many images");
    }
    let (mime, format) = detect_format(&payload.bytes, payload.mime_hint.as_deref())?;
    std::fs::create_dir_all(root).context("create generated image cache")?;
    let extension = extension_for(&mime, format);
    let file_name = format!("{}-{}.{}", Uuid::new_v4().simple(), index + 1, extension);
    let path = root.join(&file_name);
    let temporary = root.join(format!(".{file_name}.tmp"));
    std::fs::write(&temporary, &payload.bytes).context("write generated image")?;
    if let Err(error) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error).context("commit generated image");
    }
    Ok(StoredImage {
        path,
        file_name,
        mime,
        bytes: payload.bytes.len(),
    })
}

/// 按文件头判断浏览器应使用的图片媒体类型。
///
/// 参数:
/// - `bytes`: 图片字节
///
/// 返回:
/// - 图片 MIME；无法识别时返回空
pub(crate) fn content_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.is_empty() {
        return None;
    }
    // 1. 位图文件头优先。PNG 的 C2PA 元数据里可能出现 svg 字样，不能据此改类型
    if let Ok(format) = image::guess_format(bytes) {
        return raster_mime(format);
    }
    // 2. 只有文档本身以 svg 开头才当作矢量图
    looks_like_svg(bytes).then_some("image/svg+xml")
}

/// 验证图片字节并确定缓存后缀。
///
/// 参数:
/// - `bytes`: 图片字节
/// - `_mime_hint`: 供应商声明的类型，只作记录，不覆盖文件头
///
/// 返回:
/// - 媒体类型和位图格式
fn detect_format(bytes: &[u8], _mime_hint: Option<&str>) -> Result<(String, Option<ImageFormat>)> {
    let mime = content_type(bytes).context("image response is not a supported image")?;
    let format = image::guess_format(bytes).ok();
    Ok((mime.to_string(), format))
}

/// 返回已支持的位图媒体类型。
///
/// 参数:
/// - `format`: 文件头识别出的格式
///
/// 返回:
/// - 对应 MIME；不支持的格式返回空
fn raster_mime(format: ImageFormat) -> Option<&'static str> {
    match format {
        ImageFormat::Png => Some("image/png"),
        ImageFormat::Jpeg => Some("image/jpeg"),
        ImageFormat::Gif => Some("image/gif"),
        ImageFormat::WebP => Some("image/webp"),
        ImageFormat::Bmp => Some("image/bmp"),
        _ => None,
    }
}

/// 判断文本是否从文档开头声明为 SVG。
///
/// 参数:
/// - `bytes`: 待检查字节
///
/// 返回:
/// - 去掉 BOM 和 XML 声明后是否以 svg 根元素开始
fn looks_like_svg(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(512)];
    let text = String::from_utf8_lossy(sample);
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    let body = trimmed
        .strip_prefix("<?xml")
        .and_then(|value| value.split_once("?>").map(|(_, rest)| rest.trim_start()))
        .unwrap_or(trimmed);
    body.len() >= 4 && body[..4].eq_ignore_ascii_case("<svg")
}

/// 返回图片文件后缀。
fn extension_for(mime: &str, format: Option<ImageFormat>) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        _ => match format {
            Some(ImageFormat::Jpeg) => "jpg",
            Some(ImageFormat::Gif) => "gif",
            Some(ImageFormat::WebP) => "webp",
            Some(ImageFormat::Bmp) => "bmp",
            _ => "png",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_image_payload() {
        let temp = tempfile::tempdir().unwrap();
        let image = ImagePayload {
            bytes: b"not-an-image".to_vec(),
            mime_hint: Some("image/png".into()),
            source: "base64",
        };
        assert!(store(temp.path(), &image, 0).is_err());
    }

    /// 供应商 PNG 的凭据块里含有 svg 字样时，仍按 PNG 保存。
    #[test]
    fn png_with_embedded_svg_text_keeps_png_type() {
        let temp = tempfile::tempdir().unwrap();
        let mut bytes = Vec::new();
        image::DynamicImage::new_rgb8(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes.extend(br#"<svg width="1" height="1"></svg>"#);
        let stored = store(
            temp.path(),
            &ImagePayload {
                bytes,
                mime_hint: Some("image/svg+xml".into()),
                source: "base64",
            },
            0,
        )
        .unwrap();
        assert_eq!(stored.mime, "image/png");
        assert!(stored.file_name.ends_with(".png"));
    }

    /// 真正的 SVG 文档仍使用矢量类型。
    #[test]
    fn stores_svg_document_as_svg() {
        let temp = tempfile::tempdir().unwrap();
        let stored = store(
            temp.path(),
            &ImagePayload {
                bytes: br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>"#.to_vec(),
                mime_hint: None,
                source: "inline",
            },
            0,
        )
        .unwrap();
        assert_eq!(stored.mime, "image/svg+xml");
        assert!(stored.file_name.ends_with(".svg"));
    }
}

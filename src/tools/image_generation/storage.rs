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

/// 验证图片字节并优先采用供应商提供的图片 MIME。
fn detect_format(bytes: &[u8], _mime_hint: Option<&str>) -> Result<(String, Option<ImageFormat>)> {
    if bytes.is_empty() {
        bail!("image response is empty");
    }
    let text = String::from_utf8_lossy(bytes);
    let head = text
        .chars()
        .take(1024)
        .collect::<String>()
        .to_ascii_lowercase();
    if head.contains("<svg") {
        return Ok(("image/svg+xml".to_string(), None));
    }
    let format = image::guess_format(bytes).context("image response is not a supported image")?;
    let mime = match format {
        ImageFormat::Png => "image/png",
        ImageFormat::Jpeg => "image/jpeg",
        ImageFormat::Gif => "image/gif",
        ImageFormat::WebP => "image/webp",
        ImageFormat::Bmp => "image/bmp",
        _ => bail!("image response format is not supported"),
    };
    Ok((mime.to_string(), Some(format)))
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
}

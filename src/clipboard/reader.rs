use super::payload::ClipboardPayload;
use anyhow::{bail, Context, Result};
use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose, Engine as _};

/// 读取剪贴板文本或图片。
///
/// 返回:
/// - 剪贴板文本或 PNG data URL 图片
pub fn read_clipboard_payload() -> Result<ClipboardPayload> {
    let mut clipboard = Clipboard::new().context("failed to open clipboard")?;
    if let Ok(image) = clipboard.get_image() {
        return encode_clipboard_image(image);
    }
    match clipboard.get_text() {
        Ok(text) if !text.trim().is_empty() => Ok(ClipboardPayload::Text(text)),
        Ok(_) => bail!("clipboard text is empty"),
        Err(error) => bail!("clipboard has no readable text or image: {error}"),
    }
}

/// 将剪贴板图片编码为 PNG data URL。
///
/// 参数:
/// - `image`: 剪贴板图片像素数据
///
/// 返回:
/// - PNG data URL
fn encode_clipboard_image(image: ImageData<'_>) -> Result<ClipboardPayload> {
    let width = u32::try_from(image.width).context("clipboard image width is too large")?;
    let height = u32::try_from(image.height).context("clipboard image height is too large")?;
    let expected_len = image
        .width
        .checked_mul(image.height)
        .and_then(|pixels| pixels.checked_mul(4))
        .context("clipboard image dimensions are too large")?;
    if image.bytes.len() != expected_len {
        bail!(
            "clipboard image byte length mismatch: expected {expected_len}, got {}",
            image.bytes.len()
        );
    }
    let mut png_bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(image.bytes.as_ref())?;
    }
    let encoded = general_purpose::STANDARD.encode(png_bytes);
    Ok(ClipboardPayload::ImageDataUrl {
        data_url: format!("data:image/png;base64,{encoded}"),
        width: width as usize,
        height: height as usize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn encodes_clipboard_image_as_png_data_url() {
        let image = ImageData {
            width: 1,
            height: 1,
            bytes: Cow::Borrowed(&[255, 0, 0, 255]),
        };
        let payload = encode_clipboard_image(image).unwrap();
        match payload {
            ClipboardPayload::ImageDataUrl {
                data_url,
                width,
                height,
            } => {
                assert!(data_url.starts_with("data:image/png;base64,"));
                assert_eq!(width, 1);
                assert_eq!(height, 1);
            }
            _ => panic!("unexpected clipboard payload"),
        }
    }
}

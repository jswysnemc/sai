//! 图片缩放与压缩。
//!
//! 对应 CometixCode `utils/image_resizer.rs`：先把尺寸约束在 2000px 以内、字节约束在
//! 接口上限以内，再按 token 预算做二次压缩。

use super::limits::{
    format_file_size, API_IMAGE_MAX_BASE64_SIZE, IMAGE_MAX_HEIGHT, IMAGE_MAX_WIDTH,
    IMAGE_TARGET_RAW_SIZE,
};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use image::{GenericImageView, ImageEncoder};

/// 原图与发送给模型的图片尺寸。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ImageDimensions {
    pub(super) original_width: u32,
    pub(super) original_height: u32,
    pub(super) display_width: u32,
    pub(super) display_height: u32,
}

impl ImageDimensions {
    /// 判断图片是否经过缩放。
    ///
    /// 返回:
    /// - 显示尺寸与原图不同时为 true
    pub(super) fn resized(&self) -> bool {
        self.original_width != self.display_width || self.original_height != self.display_height
    }
}

/// 处理后准备发送给模型的图片。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProcessedImage {
    /// base64 编码的图片数据
    pub(super) base64: String,
    /// MIME 类型，例如 `image/png`
    pub(super) media_type: String,
    /// 缩放信息；解码失败走原图透传时为空
    pub(super) dimensions: Option<ImageDimensions>,
}

impl ProcessedImage {
    /// 返回模型请求使用的 data URL。
    ///
    /// 返回:
    /// - `data:<mime>;base64,<data>` 形式的 URL
    pub(super) fn data_url(&self) -> String {
        format!("data:{};base64,{}", self.media_type, self.base64)
    }
}

/// 按文件头魔数识别图片格式。
///
/// 参数:
/// - `bytes`: 图片字节
///
/// 返回:
/// - MIME 类型；无法识别时按 PNG 处理
pub(super) fn detect_image_format(bytes: &[u8]) -> &'static str {
    if bytes.len() < 4 {
        return "image/png";
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return "image/png";
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return "image/jpeg";
    }
    if bytes.starts_with(b"GIF") {
        return "image/gif";
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return "image/webp";
    }
    "image/png"
}

/// 把图片处理到尺寸、字节和 token 预算之内。
///
/// 1. 先做尺寸与字节约束的缩放
/// 2. 估算 token（base64 长度的八分之一）超出预算时按预算再压缩
/// 3. 预算压缩失败时退回 400x400、质量 20 的 JPEG
///
/// 参数:
/// - `bytes`: 原始图片字节
/// - `max_tokens`: 单次读取 token 上限
///
/// 返回:
/// - 可直接发送给模型的图片
pub(super) fn process_image(bytes: &[u8], max_tokens: usize) -> Result<ProcessedImage> {
    let detected = detect_image_format(bytes);
    let resized = resize_and_downsample(bytes, detected)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&resized.buffer);
    let estimated_tokens = (encoded.len() as f64 * 0.125).ceil() as usize;
    if estimated_tokens <= max_tokens {
        return Ok(ProcessedImage {
            base64: encoded,
            media_type: resized.media_type,
            dimensions: resized.dimensions,
        });
    }
    // 2. 超出 token 预算：从原图按字节预算重新压缩
    let max_bytes = ((max_tokens as f64 / 0.125).floor() * 0.75).floor() as usize;
    if let Ok(compressed) = compress_to_bytes(bytes, max_bytes, detected) {
        return Ok(compressed);
    }
    // 3. 兜底：缩到 400x400 的低质量 JPEG
    let fallback = decode(bytes)
        .and_then(|image| encode_jpeg(&shrink(&image, 400, 400), 20))
        .map(|buffer| ProcessedImage {
            base64: base64::engine::general_purpose::STANDARD.encode(buffer),
            media_type: "image/jpeg".to_string(),
            dimensions: None,
        });
    Ok(fallback.unwrap_or_else(|_| ProcessedImage {
        base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        media_type: detected.to_string(),
        dimensions: None,
    }))
}

/// 缩放结果的原始字节。
struct ResizeResult {
    buffer: Vec<u8>,
    media_type: String,
    dimensions: Option<ImageDimensions>,
}

/// 把图片约束到尺寸上限与字节目标之内。
///
/// 解码失败时，只要 base64 体积在接口上限内就原样透传。
///
/// 参数:
/// - `bytes`: 原始图片字节
/// - `detected`: 魔数识别出的 MIME 类型
///
/// 返回:
/// - 缩放后的图片字节
fn resize_and_downsample(bytes: &[u8], detected: &str) -> Result<ResizeResult> {
    if bytes.is_empty() {
        return Err(anyhow!("Image file is empty (0 bytes)"));
    }
    match try_resize(bytes, detected) {
        Ok(result) => Ok(result),
        Err(_) => {
            let encoded_size = (bytes.len() as f64 * 4.0 / 3.0).ceil() as usize;
            if encoded_size <= API_IMAGE_MAX_BASE64_SIZE && !png_over_dimensions(bytes) {
                return Ok(ResizeResult {
                    buffer: bytes.to_vec(),
                    media_type: detected.to_string(),
                    dimensions: None,
                });
            }
            if png_over_dimensions(bytes) {
                Err(anyhow!(
                    "Unable to resize image — dimensions exceed the {IMAGE_MAX_WIDTH}x{IMAGE_MAX_HEIGHT}px limit and image processing failed. Please resize the image to reduce its pixel dimensions."
                ))
            } else {
                Err(anyhow!(
                    "Unable to resize image ({} raw, {} base64). The image exceeds the 5MB API limit and compression failed. Please resize the image manually or use a smaller image.",
                    format_file_size(bytes.len() as u64),
                    format_file_size(encoded_size as u64)
                ))
            }
        }
    }
}

/// 解码并按尺寸与字节目标缩放图片。
///
/// 参数:
/// - `bytes`: 原始图片字节
/// - `detected`: 魔数识别出的 MIME 类型
///
/// 返回:
/// - 缩放结果；解码或编码失败时报错
fn try_resize(bytes: &[u8], detected: &str) -> Result<ResizeResult> {
    let image = decode(bytes)?;
    let (width, height) = image.dimensions();
    let format = format_name(bytes, detected);
    let original = |display_width, display_height| ImageDimensions {
        original_width: width,
        original_height: height,
        display_width,
        display_height,
    };
    // 1. 尺寸和字节都在范围内时原样透传
    if bytes.len() <= IMAGE_TARGET_RAW_SIZE
        && width <= IMAGE_MAX_WIDTH
        && height <= IMAGE_MAX_HEIGHT
    {
        return Ok(ResizeResult {
            buffer: bytes.to_vec(),
            media_type: mime(&format),
            dimensions: Some(original(width, height)),
        });
    }
    let needs_resize = width > IMAGE_MAX_WIDTH || height > IMAGE_MAX_HEIGHT;
    let is_png = format == "png";
    // 2. 尺寸合规但字节超标：先尝试重新编码
    if !needs_resize {
        if is_png {
            let png = encode_png(&image)?;
            if png.len() <= IMAGE_TARGET_RAW_SIZE {
                return Ok(sized(png, "png", original(width, height)));
            }
        }
        for quality in [80, 60, 40, 20] {
            let jpeg = encode_jpeg(&image, quality)?;
            if jpeg.len() <= IMAGE_TARGET_RAW_SIZE {
                return Ok(sized(jpeg, "jpeg", original(width, height)));
            }
        }
    }
    // 3. 等比缩放到尺寸上限后依次尝试原格式、PNG、各质量 JPEG
    let (target_width, target_height) = constrain_dimensions(width, height);
    let resized = shrink(&image, target_width, target_height);
    let dims = original(target_width, target_height);
    let source_format = encode_in_format(&resized, &format)?;
    if source_format.len() <= IMAGE_TARGET_RAW_SIZE {
        return Ok(sized(source_format, &format, dims));
    }
    if is_png {
        let png = encode_png(&resized)?;
        if png.len() <= IMAGE_TARGET_RAW_SIZE {
            return Ok(sized(png, "png", dims));
        }
    }
    for quality in [80, 60, 40, 20] {
        let jpeg = encode_jpeg(&resized, quality)?;
        if jpeg.len() <= IMAGE_TARGET_RAW_SIZE {
            return Ok(sized(jpeg, "jpeg", dims));
        }
    }
    // 4. 仍然超标时缩到 1000px 宽的低质量 JPEG
    let smaller_width = target_width.clamp(1, 1000);
    let smaller_height = ((target_height as u64 * smaller_width as u64 + target_width as u64 / 2)
        / target_width.max(1) as u64)
        .max(1) as u32;
    let jpeg = encode_jpeg(&shrink(&image, smaller_width, smaller_height), 20)?;
    Ok(sized(jpeg, "jpeg", original(smaller_width, smaller_height)))
}

/// 按字节预算压缩图片，逐级缩小尺寸与质量。
///
/// 参数:
/// - `bytes`: 原始图片字节
/// - `max_bytes`: 允许的最大字节数
/// - `detected`: 魔数识别出的 MIME 类型
///
/// 返回:
/// - 满足预算的图片；所有档位都失败时返回最后一档
fn compress_to_bytes(bytes: &[u8], max_bytes: usize, detected: &str) -> Result<ProcessedImage> {
    let format = format_name(bytes, detected);
    let encode = |buffer: Vec<u8>, format: &str| ProcessedImage {
        base64: base64::engine::general_purpose::STANDARD.encode(buffer),
        media_type: mime(format),
        dimensions: None,
    };
    if bytes.len() <= max_bytes {
        return Ok(encode(bytes.to_vec(), &format));
    }
    let image = decode(bytes)?;
    let (width, height) = image.dimensions();
    // 1. 同格式按比例缩小
    for factor in [1.0, 0.75, 0.5, 0.25] {
        let candidate = shrink(
            &image,
            ((width as f64 * factor).round() as u32).max(1),
            ((height as f64 * factor).round() as u32).max(1),
        );
        let buffer = match format.as_str() {
            "png" => encode_png(&candidate)?,
            "jpeg" => encode_jpeg(&candidate, 80)?,
            other => encode_in_format(&candidate, other)?,
        };
        if buffer.len() <= max_bytes {
            return Ok(encode(buffer, &format));
        }
    }
    // 2. PNG 再试 800px，其余转为中等质量 JPEG
    if format == "png" {
        let buffer = encode_png(&shrink(&image, 800, 800))?;
        if buffer.len() <= max_bytes {
            return Ok(encode(buffer, "png"));
        }
    }
    let buffer = encode_jpeg(&shrink(&image, 600, 600), 50)?;
    if buffer.len() <= max_bytes {
        return Ok(encode(buffer, "jpeg"));
    }
    Ok(encode(encode_jpeg(&shrink(&image, 400, 400), 20)?, "jpeg"))
}

/// 生成缩放信息说明，模型据此把坐标换算回原图。
///
/// 参数:
/// - `source`: 图片来源路径
/// - `processed`: 处理后的图片
/// - `original_size`: 原始文件字节数
///
/// 返回:
/// - 形如 `[Image: source: ..., ...]` 的说明
pub(super) fn image_metadata_text(
    source: &str,
    processed: &ProcessedImage,
    original_size: u64,
) -> String {
    let mut parts = vec![
        format!("source: {source}"),
        processed.media_type.clone(),
        format_file_size(original_size),
    ];
    match processed.dimensions {
        Some(dims) if dims.resized() => parts.push(format!(
            "original {}x{}, displayed at {}x{}. Multiply coordinates by {:.2} to map to original image.",
            dims.original_width,
            dims.original_height,
            dims.display_width,
            dims.display_height,
            f64::from(dims.original_width) / f64::from(dims.display_width.max(1))
        )),
        Some(dims) => parts.push(format!("{}x{}", dims.display_width, dims.display_height)),
        None => {}
    }
    format!("[Image: {}]", parts.join(", "))
}

/// 构造带尺寸信息的缩放结果。
fn sized(buffer: Vec<u8>, format: &str, dims: ImageDimensions) -> ResizeResult {
    ResizeResult {
        buffer,
        media_type: mime(format),
        dimensions: Some(dims),
    }
}

/// 解码图片，不启用 image crate 默认的尺寸与内存限制。
fn decode(bytes: &[u8]) -> Result<image::DynamicImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    reader.limits(image::Limits::no_limits());
    Ok(reader.decode()?)
}

/// 返回图片的格式名（png/jpeg/gif/webp）。
fn format_name(bytes: &[u8], detected: &str) -> String {
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Jpeg) => "jpeg".to_string(),
        Ok(image::ImageFormat::Png) => "png".to_string(),
        Ok(image::ImageFormat::Gif) => "gif".to_string(),
        Ok(image::ImageFormat::WebP) => "webp".to_string(),
        _ => detected.trim_start_matches("image/").to_string(),
    }
}

/// 格式名转 MIME 类型。
fn mime(format: &str) -> String {
    format!("image/{format}")
}

/// 在不放大的前提下把图片缩到指定边界内。
fn shrink(image: &image::DynamicImage, width: u32, height: u32) -> image::DynamicImage {
    let (current_width, current_height) = image.dimensions();
    if current_width <= width && current_height <= height {
        image.clone()
    } else {
        image.resize(
            width.max(1),
            height.max(1),
            image::imageops::FilterType::Lanczos3,
        )
    }
}

/// 等比约束宽高到 2000px 上限。
fn constrain_dimensions(mut width: u32, mut height: u32) -> (u32, u32) {
    if width > IMAGE_MAX_WIDTH {
        height = ((height as u64 * IMAGE_MAX_WIDTH as u64 + width as u64 / 2) / width as u64).max(1)
            as u32;
        width = IMAGE_MAX_WIDTH;
    }
    if height > IMAGE_MAX_HEIGHT {
        width = ((width as u64 * IMAGE_MAX_HEIGHT as u64 + height as u64 / 2) / height as u64)
            .max(1) as u32;
        height = IMAGE_MAX_HEIGHT;
    }
    (width.max(1), height.max(1))
}

/// 判断 PNG 文件头声明的尺寸是否超出上限。
fn png_over_dimensions(bytes: &[u8]) -> bool {
    bytes.len() >= 24
        && bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        && (u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) > IMAGE_MAX_WIDTH
            || u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) > IMAGE_MAX_HEIGHT)
}

/// 编码为 PNG。
fn encode_png(image: &image::DynamicImage) -> Result<Vec<u8>> {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut output = Vec::new();
    image::codecs::png::PngEncoder::new(&mut output)
        .write_image(
            rgba.as_raw(),
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .context("encode png")?;
    Ok(output)
}

/// 编码为指定质量的 JPEG。
fn encode_jpeg(image: &image::DynamicImage, quality: u8) -> Result<Vec<u8>> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    let mut output = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
        .encode(rgb.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .context("encode jpeg")?;
    Ok(output)
}

/// 按源格式重新编码，未知格式按 PNG 处理。
fn encode_in_format(image: &image::DynamicImage, format: &str) -> Result<Vec<u8>> {
    match format {
        "jpeg" => encode_jpeg(image, 80),
        "gif" | "webp" => {
            let target = if format == "gif" {
                image::ImageFormat::Gif
            } else {
                image::ImageFormat::WebP
            };
            let mut output = std::io::Cursor::new(Vec::new());
            image.write_to(&mut output, target)?;
            Ok(output.into_inner())
        }
        _ => encode_png(image),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成纯色 PNG 字节。
    fn png(width: u32, height: u32) -> Vec<u8> {
        let rgba = image::RgbaImage::from_pixel(width, height, image::Rgba([16, 32, 48, 255]));
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                rgba.as_raw(),
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        bytes
    }

    /// 小图原样透传并记录尺寸。
    #[test]
    fn small_image_passes_through() {
        let bytes = png(40, 20);
        let processed = process_image(&bytes, 25_000).unwrap();
        assert_eq!(processed.media_type, "image/png");
        let dims = processed.dimensions.unwrap();
        assert_eq!((dims.display_width, dims.display_height), (40, 20));
        assert!(!dims.resized());
    }

    /// 超宽图片等比缩放到 2000px，并给出坐标换算说明。
    #[test]
    fn oversized_image_is_resized_with_scale_note() {
        let bytes = png(2500, 500);
        let processed = process_image(&bytes, 25_000).unwrap();
        let dims = processed.dimensions.unwrap();
        assert_eq!((dims.display_width, dims.display_height), (2000, 400));
        let text = image_metadata_text("/tmp/wide.png", &processed, bytes.len() as u64);
        assert!(
            text.contains("original 2500x500, displayed at 2000x400"),
            "{text}"
        );
        assert!(text.contains("Multiply coordinates by 1.25"), "{text}");
    }

    /// 魔数识别覆盖常见格式，过短数据按 PNG 处理。
    #[test]
    fn detects_formats_by_magic_bytes() {
        assert_eq!(detect_image_format(&[0xff, 0xd8, 0xff, 0x00]), "image/jpeg");
        assert_eq!(detect_image_format(b"GIF89a"), "image/gif");
        assert_eq!(detect_image_format(b"RIFF\0\0\0\0WEBPVP8 "), "image/webp");
        assert_eq!(detect_image_format(&[0xff]), "image/png");
    }

    /// 空文件直接报错。
    #[test]
    fn empty_image_is_rejected() {
        assert!(process_image(&[], 25_000).is_err());
    }

    /// token 预算极小时压缩到预算之内。
    #[test]
    fn tight_token_budget_compresses_image() {
        let bytes = png(1200, 1200);
        let processed = process_image(&bytes, 200).unwrap();
        assert!(
            processed.base64.len() as f64 * 0.125 <= 200.0 || processed.media_type == "image/jpeg"
        );
    }
}

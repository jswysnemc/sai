use super::{
    kitty_image_id, kitty_placement_payload, kitty_transmission_payload, next_kitty_placement_id,
    supports_iterm_inline_image, supports_kitty_graphics, supports_windows_terminal_sixel,
    terminal_cell_pixel_size,
};
use anyhow::{bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageReader, Rgb, RgbImage};
use std::io::{Cursor, Read};

const COLUMNS: usize = 6;
const ROWS: usize = 3;
const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 1024;
const MAX_IMAGE_SIDE: u32 = 16384;
const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TILE_SIDE: usize = 512;
const BACKGROUND: Rgb<u8> = Rgb([38, 38, 38]);
const BORDER: Rgb<u8> = Rgb([128, 128, 128]);

/// 【输入缩略图】【缓冲产物】可缓存重放的协议内容及当前放置的清理指令。
/// 参数: 无；返回: payload 不含布局换行，cleanup 在非 Kitty 协议下为空。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComposerTile {
    pub(crate) payload: String,
    pub(crate) cleanup: String,
}

#[derive(Clone, Copy)]
enum Protocol {
    Kitty,
    Iterm,
    Sixel,
}

impl Protocol {
    /// 【输入缩略图】【协议选择】沿用正文图片的能力检测及优先级。
    /// 参数: 无；返回: 当前可用图形协议，不支持时返回 None。
    fn detect() -> Option<Self> {
        if supports_kitty_graphics() {
            Some(Self::Kitty)
        } else if supports_iterm_inline_image() {
            Some(Self::Iterm)
        } else if supports_windows_terminal_sixel() {
            Some(Self::Sixel)
        } else {
            None
        }
    }
}

/// 【输入缩略图】【环境尺寸】查询真实单元格像素，供调用方纳入缓存键。
/// 参数: 无；返回: 支持图形协议时为单元格宽高，缺失尺寸回退 8×16，否则 None。
/// 不套用正文的等宽字体比例归一化；异常巨大尺寸由渲染入口拒绝。
pub(crate) fn composer_tile_environment() -> Option<(usize, usize)> {
    Protocol::detect().map(|_| terminal_cell_pixel_size())
}

/// 【输入缩略图】【缓冲渲染】生成六列三行、等比居中的不透明缩略图协议。
/// 参数: source 为图片 data URL（base64 或百分号编码）或本地普通文件路径。
/// 返回: 完整协议及定向清理指令；不支持图形、输入超限或解码失败时返回错误。
/// 文件/解码分别限制为 16/64 MiB，原图最多 16M 像素，目标栅格每边最多 512 像素。
/// 此函数不写 stdout、不登记正文传输缓存；非 Kitty 图片由调用方重绘背景清除。
pub(crate) fn render_composer_tile(source: &str) -> Result<ComposerTile> {
    let protocol = Protocol::detect().context("terminal does not support composer images")?;
    let dimensions = tile_dimensions(terminal_cell_pixel_size())?;
    let bytes = read_source(source)?;
    let image = decode_image(&bytes)?;
    let canvas = compose_image(image, dimensions);
    encode_tile(&canvas, protocol)
}

/// 【输入缩略图】【栅格限额】检查真实单元格尺寸，避免乘法溢出或巨大画布。
/// 参数: cell 为终端单元格像素；返回: 六列三行的实际像素宽高，超限报错。
fn tile_dimensions(cell: (usize, usize)) -> Result<(u32, u32)> {
    let width = cell.0.checked_mul(COLUMNS).context("tile width overflow")?;
    let height = cell.1.checked_mul(ROWS).context("tile height overflow")?;
    ensure!(
        (3..=MAX_TILE_SIDE).contains(&width) && (3..=MAX_TILE_SIDE).contains(&height),
        "composer tile pixel dimensions exceed limits"
    );
    Ok((width as u32, height as u32))
}

/// 【输入缩略图】【有界读取】读取图片数据而不根据外部长度无限分配。
/// 参数: source 为 data URL 或普通文件路径；返回: 最多 16 MiB 的非空图片字节。
fn read_source(source: &str) -> Result<Vec<u8>> {
    let bytes = if let Some(data) = source.strip_prefix("data:") {
        ensure!(
            data.len() <= MAX_SOURCE_BYTES * 3 + MAX_HEADER_BYTES,
            "image data URL exceeds size limit"
        );
        let (header, encoded) = data.split_once(',').context("invalid image data URL")?;
        ensure!(
            header.len() <= MAX_HEADER_BYTES,
            "image data header too long"
        );
        let mut fields = header.split(';');
        let mime = fields.next().unwrap_or_default();
        ensure!(
            mime.get(..6)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("image/"))
                && mime.len() > 6,
            "data URL must have an image media type"
        );
        let base64 = fields.any(|field| field.eq_ignore_ascii_case("base64"));
        if base64 {
            // 1. 【输入缩略图】【预分配限额】先限制编码长度，再做百分号还原和 base64 解码
            let encoded_limit = MAX_SOURCE_BYTES.div_ceil(3) * 4;
            let decoded_url = percent_decode(encoded, encoded_limit)?;
            STANDARD
                .decode(decoded_url)
                .context("invalid image base64")?
        } else {
            percent_decode(encoded, MAX_SOURCE_BYTES)?
        }
    } else {
        ensure!(source.len() <= 32 * 1024, "image path exceeds size limit");
        let metadata = std::fs::metadata(source).context("inspect composer image file")?;
        ensure!(metadata.is_file(), "composer image must be a regular file");
        ensure!(
            metadata.len() <= MAX_SOURCE_BYTES as u64,
            "image file too large"
        );
        let file = std::fs::File::open(source).context("open composer image file")?;
        let metadata = file.metadata().context("inspect opened composer image")?;
        ensure!(metadata.is_file(), "composer image must be a regular file");
        ensure!(
            metadata.len() <= MAX_SOURCE_BYTES as u64,
            "image file too large"
        );
        // 1. 【输入缩略图】【增长防护】即使文件在检查后增长，也只读取上限加一个哨兵字节
        let mut bytes = Vec::new();
        file.take(MAX_SOURCE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        bytes
    };
    ensure!(!bytes.is_empty(), "empty composer image");
    ensure!(
        bytes.len() <= MAX_SOURCE_BYTES,
        "image data exceeds size limit"
    );
    Ok(bytes)
}

/// 【输入缩略图】【URL 字节】严格还原百分号转义，不把加号误当成空格。
/// 参数: value 为 URL 数据部分，limit 为还原字节上限；返回: 有界字节或格式错误。
fn percent_decode(value: &str, limit: usize) -> Result<Vec<u8>> {
    ensure!(
        value.len() <= limit.saturating_mul(3),
        "encoded image data too large"
    );
    let mut output = Vec::with_capacity(value.len().min(limit));
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        ensure!(output.len() < limit, "decoded image data too large");
        output.push(if byte == b'%' {
            let high = input.next().and_then(|b| char::from(b).to_digit(16));
            let low = input.next().and_then(|b| char::from(b).to_digit(16));
            match (high, low) {
                (Some(high), Some(low)) => (high * 16 + low) as u8,
                _ => bail!("invalid image URL percent escape"),
            }
        } else {
            byte
        });
    }
    Ok(output)
}

/// 【输入缩略图】【安全解码】先验证尺寸和输出内存，再解码首帧并应用图片方向。
/// 参数: bytes 为已限长的图片文件内容；返回: 方向校正后的图片或解码错误。
fn decode_image(bytes: &[u8]) -> Result<DynamicImage> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    reader.limits(limits.clone());
    let mut decoder = reader
        .into_decoder()
        .context("open bounded image decoder")?;
    let (width, height) = decoder.dimensions();
    ensure!(width > 0 && height > 0, "empty image dimensions");
    ensure!(
        u64::from(width) * u64::from(height) <= MAX_IMAGE_PIXELS,
        "decoded image pixel count exceeds limit"
    );
    // 1. 【输入缩略图】【双重限额】部分解码器的 max_alloc 是软限额，独立检查输出字节并预留预算
    limits
        .reserve(decoder.total_bytes())
        .context("decoded image memory exceeds limit")?;
    decoder.set_limits(limits)?;
    let orientation = decoder.orientation().context("read image orientation")?;
    let mut image = DynamicImage::from_decoder(decoder).context("decode composer image")?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// 【输入缩略图】【画布合成】等比缩放到边框内部，居中并将透明像素混合到深灰背景。
/// 参数: image 为校正方向的图片，dimensions 为已验证画布宽高；返回: 带一像素灰边的 RGB 图。
fn compose_image(image: DynamicImage, dimensions: (u32, u32)) -> RgbImage {
    let (width, height) = dimensions;
    let thumbnail = image.thumbnail(width - 2, height - 2).to_rgba8();
    let mut canvas = RgbImage::from_pixel(width, height, BACKGROUND);
    let left = (width - thumbnail.width()) / 2;
    let top = (height - thumbnail.height()) / 2;
    for (x, y, pixel) in thumbnail.enumerate_pixels() {
        let alpha = u32::from(pixel[3]);
        let mut blended = [0; 3];
        for channel in 0..3 {
            blended[channel] = ((u32::from(pixel[channel]) * alpha
                + u32::from(BACKGROUND[channel]) * (255 - alpha)
                + 127)
                / 255) as u8;
        }
        canvas.put_pixel(left + x, top + y, Rgb(blended));
    }
    for x in 0..width {
        canvas.put_pixel(x, 0, BORDER);
        canvas.put_pixel(x, height - 1, BORDER);
    }
    for y in 0..height {
        canvas.put_pixel(0, y, BORDER);
        canvas.put_pixel(width - 1, y, BORDER);
    }
    canvas
}

/// 【输入缩略图】【协议封装】复用正文的纯编码工具，不产生终端或缓存副作用。
/// 参数: canvas 为有界 RGB 画布，protocol 为输出协议；返回: 无布局换行的载荷和清理串。
fn encode_tile(canvas: &RgbImage, protocol: Protocol) -> Result<ComposerTile> {
    if matches!(protocol, Protocol::Sixel) {
        let raster = super::RasterImage {
            width: canvas.width() as usize,
            height: canvas.height() as usize,
            pixels: canvas
                .pixels()
                .map(|p| super::Rgba {
                    r: p[0],
                    g: p[1],
                    b: p[2],
                    a: 255,
                })
                .collect(),
        };
        let pixels = super::quantize_for_sixel(&raster, raster.width, raster.height);
        let mut payload = super::encode_sixel(&pixels, raster.width, raster.height);
        // 1. 【输入缩略图】【布局隔离】共享编码器自带正文换行，缩略图由输入框控制占位
        if payload.ends_with('\n') {
            payload.pop();
        }
        return Ok(ComposerTile {
            payload,
            cleanup: String::new(),
        });
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(
        canvas.as_raw(),
        canvas.width(),
        canvas.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    match protocol {
        Protocol::Kitty => {
            let image_id = kitty_image_id(&png);
            let placement_id = next_kitty_placement_id();
            let mut payload = kitty_transmission_payload(image_id, &png);
            payload.push_str(&kitty_placement_payload(
                image_id, placement_id, Some(COLUMNS), Some(ROWS),
            ));
            // 1. 【输入缩略图】【定向清理】小写 d=i 配合 i/p 仅删该放置，不释放共享图像或删除正文
            let cleanup = format!("\x1b_Ga=d,d=i,q=2,i={image_id},p={placement_id}\x1b\\");
            Ok(ComposerTile { payload, cleanup })
        }
        Protocol::Iterm => Ok(ComposerTile {
            payload: format!(
                "\x1b]1337;File=inline=1;width={COLUMNS};height={ROWS};preserveAspectRatio=1:{}\x07",
                STANDARD.encode(png)
            ),
            cleanup: String::new(),
        }),
        Protocol::Sixel => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct EnvironmentGuard;

    impl EnvironmentGuard {
        fn set(kitty: bool, iterm: bool, sixel: bool) -> Self {
            super::super::test_override::set(Some(kitty), Some(iterm), Some(sixel));
            Self
        }
    }

    impl Drop for EnvironmentGuard {
        fn drop(&mut self) {
            super::super::test_override::set(None, None, None);
        }
    }

    fn png_bytes(image: &RgbImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        bytes
    }

    fn data_url(bytes: &[u8]) -> String {
        format!("data:image/png;base64,{}", STANDARD.encode(bytes))
    }

    #[test]
    fn environment_requires_graphics_and_preserves_actual_cell_pixels() {
        for flags in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let _guard = EnvironmentGuard::set(flags.0, flags.1, flags.2);
            assert_eq!(
                composer_tile_environment(),
                Some(terminal_cell_pixel_size())
            );
        }
        let _guard = EnvironmentGuard::set(false, false, false);
        assert_eq!(composer_tile_environment(), None);
        assert!(render_composer_tile("missing.png").is_err());
    }

    #[test]
    fn raster_uses_real_cells_and_rejects_giant_sizes() {
        assert_eq!(tile_dimensions((8, 16)).unwrap(), (48, 48));
        assert_eq!(tile_dimensions((12, 20)).unwrap(), (72, 60));
        for cell in [(usize::MAX, 16), (8, usize::MAX), (1000, 1000), (0, 16)] {
            assert!(tile_dimensions(cell).is_err());
        }
    }

    #[test]
    fn aspect_ratio_centering_background_and_one_pixel_border() {
        for (source, expected) in [((80, 40), (46, 23)), ((40, 80), (23, 46))] {
            let image = RgbImage::from_pixel(source.0, source.1, Rgb([255, 0, 0]));
            let canvas = compose_image(DynamicImage::ImageRgb8(image), (48, 48));
            let left = (48 - expected.0) / 2;
            let top = (48 - expected.1) / 2;
            for (x, y, pixel) in canvas.enumerate_pixels() {
                let expected_pixel = if x == 0 || x == 47 || y == 0 || y == 47 {
                    BORDER
                } else if (left..left + expected.0).contains(&x)
                    && (top..top + expected.1).contains(&y)
                {
                    Rgb([255, 0, 0])
                } else {
                    BACKGROUND
                };
                assert_eq!(*pixel, expected_pixel, "at {x},{y}");
            }
        }
    }

    #[test]
    fn transparency_is_composited_onto_background() {
        for (alpha, expected) in [(0, BACKGROUND), (128, Rgb([147, 19, 19]))] {
            let image = image::RgbaImage::from_pixel(46, 46, image::Rgba([255, 0, 0, alpha]));
            let canvas = compose_image(DynamicImage::ImageRgba8(image), (48, 48));
            assert_eq!(*canvas.get_pixel(24, 24), expected);
            assert_eq!(*canvas.get_pixel(0, 24), BORDER);
        }
    }

    #[test]
    fn source_accepts_local_base64_and_percent_encoded_images() {
        let bytes = png_bytes(&RgbImage::from_pixel(8, 4, BACKGROUND));
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&bytes).unwrap();
        assert_eq!(read_source(file.path().to_str().unwrap()).unwrap(), bytes);
        assert_eq!(read_source(&data_url(&bytes)).unwrap(), bytes);
        let encoded: String = bytes.iter().map(|byte| format!("%{byte:02X}")).collect();
        assert_eq!(
            read_source(&format!("data:image/png,{encoded}")).unwrap(),
            bytes
        );
        assert_eq!(percent_decode("+%2B", 2).unwrap(), b"++");
        assert_eq!(decode_image(&bytes).unwrap().width(), 8);
    }

    #[test]
    fn invalid_and_oversized_sources_are_rejected() {
        for source in [
            "data:text/plain;base64,AAAA",
            "data:image/png;base64,!",
            "data:image/png,%GG",
            "data:image/png,%0",
            "data:image/png,",
            "data:image/png",
        ] {
            assert!(read_source(source).is_err(), "{source}");
        }
        assert!(decode_image(b"not an image").is_err());
        assert!(percent_decode("12345", 4).is_err());
        let directory = tempfile::tempdir().unwrap();
        assert!(read_source(directory.path().to_str().unwrap()).is_err());
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(MAX_SOURCE_BYTES as u64 + 1).unwrap();
        assert!(read_source(file.path().to_str().unwrap()).is_err());
        let data = format!("data:image/png,{}", "a".repeat(MAX_SOURCE_BYTES + 1));
        assert!(read_source(&data).is_err());
        let encoded = format!(
            "data:image/png;base64,{}",
            "A".repeat(MAX_SOURCE_BYTES.div_ceil(3) * 4 + 4)
        );
        assert!(read_source(&encoded).is_err());
    }

    #[test]
    fn huge_image_headers_fail_before_pixel_allocation() {
        // 1. 【输入缩略图】【压缩炸弹】BMP 只提供头部，必须在尝试读取巨大像素缓冲前拒绝
        for (width, height) in [(MAX_IMAGE_SIDE + 1, 1u32), (8192, 8192)] {
            let mut bmp = vec![0u8; 54];
            bmp[..2].copy_from_slice(b"BM");
            bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
            bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
            bmp[18..22].copy_from_slice(&width.to_le_bytes());
            bmp[22..26].copy_from_slice(&height.to_le_bytes());
            bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
            bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
            let error = format!("{:#}", decode_image(&bmp).unwrap_err());
            assert!(
                error.contains("limit") || error.contains("Limit"),
                "{error}"
            );
        }
    }

    #[test]
    fn decoder_applies_exif_orientation() {
        let image = RgbImage::from_pixel(12, 6, Rgb([240, 20, 30]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode_image(&image)
            .unwrap();
        // 1. 【输入缩略图】【方向样本】Exif orientation=6 表示顺时针旋转九十度
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut oriented = jpeg[..2].to_vec();
        oriented.extend_from_slice(b"\xff\xe1");
        oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        oriented.extend_from_slice(exif);
        oriented.extend_from_slice(&jpeg[2..]);
        let decoded = decode_image(&oriented).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (6, 12));
    }

    #[test]
    fn kitty_transmits_png_and_cleans_only_its_unique_placement() {
        let canvas = RgbImage::from_pixel(48, 48, BACKGROUND);
        let bytes = png_bytes(&canvas);
        let image_id = kitty_image_id(&bytes);
        let first = encode_tile(&canvas, Protocol::Kitty).unwrap();
        let second = encode_tile(&canvas, Protocol::Kitty).unwrap();
        let prefix = kitty_transmission_payload(image_id, &bytes);
        assert!(first.payload.starts_with(&prefix));
        let placement = first.payload.strip_prefix(&prefix).unwrap();
        assert!(placement.starts_with(&format!("\x1b_Ga=p,q=2,C=1,i={image_id},p=")));
        assert!(placement.ends_with(",c=6,r=3\x1b\\"));
        let placement_id = placement
            .split("p=")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        assert_eq!(
            first.cleanup,
            format!("\x1b_Ga=d,d=i,q=2,i={image_id},p={placement_id}\x1b\\")
        );
        assert_ne!(first.cleanup, second.cleanup);
        assert_ne!(first.payload, second.payload);
        assert_eq!(first.clone(), first);
        assert!(!first.payload.contains('\n'));
    }

    #[test]
    fn iterm_declares_grid_and_embeds_complete_png() {
        let canvas = compose_image(DynamicImage::ImageRgb8(RgbImage::new(20, 10)), (72, 60));
        let tile = encode_tile(&canvas, Protocol::Iterm).unwrap();
        let prefix = "\x1b]1337;File=inline=1;width=6;height=3;preserveAspectRatio=1:";
        let encoded = tile
            .payload
            .strip_prefix(prefix)
            .unwrap()
            .strip_suffix('\x07')
            .unwrap();
        let decoded = image::load_from_memory(&STANDARD.decode(encoded).unwrap())
            .unwrap()
            .to_rgb8();
        assert_eq!(decoded, canvas);
        assert!(tile.cleanup.is_empty());
    }

    #[test]
    fn sixel_uses_actual_pixels_and_has_no_layout_newline() {
        let canvas = RgbImage::from_pixel(72, 60, BACKGROUND);
        let tile = encode_tile(&canvas, Protocol::Sixel).unwrap();
        assert!(tile.payload.starts_with("\x1bPq\"1;1;72;60"));
        assert!(tile.payload.ends_with("\x1b\\"));
        assert!(!tile.payload.contains('\n'));
        assert!(tile.cleanup.is_empty());
    }

    #[test]
    fn public_renderer_uses_each_supported_protocol() {
        let source = data_url(&png_bytes(&RgbImage::from_pixel(20, 10, BACKGROUND)));
        for (flags, prefix) in [
            ((true, true, true), "\x1b_Ga=t"),
            ((false, true, true), "\x1b]1337;"),
            ((false, false, true), "\x1bPq"),
        ] {
            let _guard = EnvironmentGuard::set(flags.0, flags.1, flags.2);
            let tile = render_composer_tile(&source).unwrap();
            assert!(tile.payload.starts_with(prefix));
        }
    }
}

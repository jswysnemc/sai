//! 工具截图：按页面区域捕获图像，并在浏览器之外转换为 CSS 像素尺寸。

use super::session::BrowserSession;
use anyhow::{Context, Result};
use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use serde_json::{json, Value};

/// 整页截图的最大 CSS 高度，避免超长页面生成巨幅图片。
const FULL_PAGE_MAX_HEIGHT: f64 = 8_000.0;
const SCREENSHOT_QUALITY: u8 = 85;

impl BrowserSession {
    /// 【内置浏览器】【页面截图】截取视口、整页或单个元素，返回 CSS 像素尺寸的 JPEG。
    /// @param full_page 为是否整页；reference 为可选元素 ref
    /// @returns JPEG 字节与截图说明
    pub(crate) async fn screenshot(
        &self,
        full_page: bool,
        reference: Option<&str>,
    ) -> Result<(Vec<u8>, String)> {
        let scale = self.device_scale();
        let mut params = json!({ "format": "jpeg", "quality": SCREENSHOT_QUALITY });
        // 1. 【内置浏览器】【截图坐标】裁剪使用文档 CSS 坐标，保留浏览器当前渲染密度
        let description = if let Some(reference) = reference {
            let element = self.locate(reference).await?;
            // 【内置浏览器】【元素截图】定位会把元素滚动到可见区，因此在定位之后读取文档偏移
            let metrics = self.page_send("Page.getLayoutMetrics", json!({})).await?;
            let viewport = &metrics["cssVisualViewport"];
            let x = viewport["pageX"].as_f64().unwrap_or(0.0);
            let y = viewport["pageY"].as_f64().unwrap_or(0.0);
            params["clip"] = json!({
                "x": x + (element.x - element.width / 2.0).max(0.0),
                "y": y + (element.y - element.height / 2.0).max(0.0),
                "width": element.width,
                "height": element.height,
                "scale": 1,
            });
            format!("element {} [ref={reference}]", element.tag)
        } else if full_page {
            let metrics = self.page_send("Page.getLayoutMetrics", json!({})).await?;
            let size = metrics
                .get("cssContentSize")
                .or_else(|| metrics.get("contentSize"))
                .cloned()
                .unwrap_or(Value::Null);
            let width = size["width"].as_f64().unwrap_or(1280.0);
            let height = size["height"]
                .as_f64()
                .unwrap_or(800.0)
                .min(FULL_PAGE_MAX_HEIGHT);
            params["clip"] =
                json!({ "x": 0, "y": 0, "width": width, "height": height, "scale": 1 });
            params["captureBeyondViewport"] = json!(true);
            format!("full page {width:.0}x{height:.0}")
        } else {
            let (width, height) = self.viewport();
            params["captureBeyondViewport"] = json!(false);
            format!("viewport {width}x{height}")
        };
        // 2. 【内置浏览器】【截图压缩】需要缩小时先捕获无损图像，避免重复 JPEG 压缩
        if scale > 1.0 {
            params["format"] = json!("png");
            params.as_object_mut().unwrap().remove("quality");
        }
        let result = self.page_send("Page.captureScreenshot", params).await?;
        let data = result["data"]
            .as_str()
            .context("screenshot returned no data")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("decode screenshot")?;
        // 3. 【内置浏览器】【截图缩放】在工作线程转换像素尺寸，不修改录屏所用的浏览器渲染状态
        let bytes = if scale > 1.0 {
            tokio::task::spawn_blocking(move || normalize_pixels(bytes, scale))
                .await
                .context("resize screenshot task")??
        } else {
            bytes
        };
        Ok((bytes, description))
    }
}

/// 【内置浏览器】【截图缩放】把设备像素图像转换为 CSS 像素 JPEG。
/// @param bytes 为原始图像字节；scale 为捕获时的设备像素比
/// @returns 缩放后 JPEG 字节
fn normalize_pixels(bytes: Vec<u8>, scale: f64) -> Result<Vec<u8>> {
    let source = image::load_from_memory(&bytes).context("decode screenshot pixels")?;
    let width = (f64::from(source.width()) / scale).round().max(1.0) as u32;
    let height = (f64::from(source.height()) / scale).round().max(1.0) as u32;
    let resized = source.resize_exact(width, height, FilterType::Lanczos3);
    let mut output = Vec::new();
    JpegEncoder::new_with_quality(&mut output, SCREENSHOT_QUALITY)
        .encode_image(&resized)
        .context("encode screenshot JPEG")?;
    Ok(output)
}

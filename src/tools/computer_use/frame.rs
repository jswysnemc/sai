//! 截图帧的交付像素与原始窗口像素映射，不使用桌面绝对坐标。

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::io::Cursor;

/// 【桌面操作】【截图帧】保存源图片尺寸、裁剪区域和交付尺寸，避免缩放后的点击偏移。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ScreenshotFrame {
    /// 驱动交付的完整窗口截图宽高，单位为像素。
    source: (u32, u32),
    /// 裁剪区域左上角与宽高，单位为源截图像素。
    region: (u32, u32, u32, u32),
    /// 交给模型的图片宽高，单位为交付像素。
    pub(super) image: (u32, u32),
}

impl ScreenshotFrame {
    /// 【桌面操作】【帧构造】按实际源图片校验裁剪区域，并计算有界缩放后的尺寸。
    /// 参数: `source` 为源图片宽高，`args` 为观察参数；返回: 有效帧或区域越界错误。
    pub(super) fn new(source: (u32, u32), args: &Value) -> Result<Self> {
        // 1. 【桌面操作】【区域校验】裁剪区域必须完整位于驱动提供的窗口原图内
        let region = if let Some(region) = args.get("region") {
            let field = |key| -> Result<u32> {
                u32::try_from(super::schema::integer(region, key)?)
                    .context("screenshot region is too large")
            };
            (field("x")?, field("y")?, field("width")?, field("height")?)
        } else {
            (0, 0, source.0, source.1)
        };
        if source.0 == 0
            || source.1 == 0
            || region.2 == 0
            || region.3 == 0
            || region
                .0
                .checked_add(region.2)
                .is_none_or(|end| end > source.0)
            || region
                .1
                .checked_add(region.3)
                .is_none_or(|end| end > source.1)
        {
            bail!(
                "screenshot_region_out_of_bounds: region must fit inside the {}x{} window image",
                source.0,
                source.1
            );
        }
        // 2. 【桌面操作】【交付尺寸】保留实际取整后的尺寸，用于后续坐标投影
        let scale = args.get("scale").and_then(Value::as_f64).unwrap_or(1.0);
        if !scale.is_finite() || !(0.1..=1.0).contains(&scale) {
            bail!("screenshot scale must be between 0.1 and 1.0");
        }
        Ok(Self {
            source,
            region,
            image: (
                (f64::from(region.2) * scale).round().max(1.0) as u32,
                (f64::from(region.3) * scale).round().max(1.0) as u32,
            ),
        })
    }

    /// 【桌面操作】【坐标投影】将同一帧内的交付像素投影回驱动原图，严格拒绝边界外坐标。
    /// 参数: `x/y` 为模型看到的图内坐标；返回: 对应源截图的窗口局部坐标。
    pub(super) fn project(&self, x: f64, y: f64) -> Result<(f64, f64)> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= f64::from(self.image.0)
            || y >= f64::from(self.image.1)
        {
            bail!(
                "coordinates_out_of_bounds: point must be inside the {}x{} screenshot",
                self.image.0,
                self.image.1
            );
        }
        Ok((
            f64::from(self.region.0) + x * f64::from(self.region.2) / f64::from(self.image.0),
            f64::from(self.region.1) + y * f64::from(self.region.3) / f64::from(self.image.1),
        ))
    }

    /// 【桌面操作】【图片变换】需要裁剪或缩放时重新编码为 PNG，默认观察保持原字节。
    /// 参数: `bytes` 为已校验的源图片；返回: 变换后的 PNG，未变换时为空。
    pub(super) fn transform(&self, bytes: &[u8]) -> Result<Option<Vec<u8>>> {
        // 1. 【桌面操作】【默认图片】未变换时沿用原字节，避免额外编码损失
        if self.region == (0, 0, self.source.0, self.source.1) && self.image == self.source {
            return Ok(None);
        }
        // 2. 【桌面操作】【图片变换】先按原图裁剪，再缩放到记录的交付尺寸
        let image =
            image::load_from_memory(bytes).context("cannot decode Computer Use screenshot")?;
        let cropped = image.crop_imm(self.region.0, self.region.1, self.region.2, self.region.3);
        let resized = cropped.resize_exact(
            self.image.0,
            self.image.1,
            image::imageops::FilterType::Triangle,
        );
        let mut output = Cursor::new(Vec::new());
        resized.write_to(&mut output, image::ImageFormat::Png)?;
        Ok(Some(output.into_inner()))
    }

    /// 【桌面操作】【帧说明】描述交付图及裁剪来源，明确所有输入仍以交付图为坐标基准。
    /// 参数: 无；返回: 可交给模型的帧尺寸说明。
    pub(super) fn description(&self) -> String {
        format!("Screenshot: {}x{}; use pixels from this exact image. Source window image: {}x{}; region: {},{} {}x{}. Sai projects image coordinates back to the source window.",
            self.image.0, self.image.1, self.source.0, self.source.1,
            self.region.0, self.region.1, self.region.2, self.region.3)
    }
}

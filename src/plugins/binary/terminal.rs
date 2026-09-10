use crate::media::terminal;
use crate::plugins::system::paths::{expand, workdir};
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::{
    host::{DisplayedImage, SystemContext},
    Capabilities,
};
use std::io::{Read, Write};
use std::path::Path;

/// 【插件图片】【绘制宿主】只允许图片展示授权，读取与渲染在线程内完成，取消后不提交终端输出。
/// @param path 图片路径；size 为显示尺寸；context 为可信工作目录；capabilities 为展示授权
/// @returns 原图片绝对路径，不返回原始图片字节
pub(in crate::plugins) async fn display(
    path: String,
    size: Option<String>,
    context: SystemContext,
    capabilities: Capabilities,
) -> Result<DisplayedImage> {
    if !capabilities.binary.display_images {
        bail!("plugin image display is not allowed");
    }
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        bail!("invalid plugin image path");
    }
    let path = expand(path.trim(), &workdir(&context)?)?;
    let reported = path.to_string_lossy().into_owned();
    let size = bounded_size(size.as_deref());
    let rendered = tokio::task::spawn_blocking(move || prepare(&path, size.as_deref()))
        .await
        .context("plugin image renderer stopped")??;
    terminal::print_rendered(&rendered)?;
    Ok(DisplayedImage { path: reported })
}

/// 【插件图片】【有界快照】读取普通文件快照并检查图片像素数，原路径更换不会改变渲染输入。
/// @param path 图片路径；size 为已经限制的单元格尺寸
/// @returns 完整终端协议内容
fn prepare(path: &Path, size: Option<&str>) -> Result<String> {
    let metadata = std::fs::metadata(path).context("failed to stat image")?;
    if !metadata.is_file() {
        bail!("image path is not a file");
    }
    if metadata.len() > 64 * 1024 * 1024 {
        bail!("plugin image file exceeds byte limit");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let mut input = options.open(path).context("failed to open image")?;
    if !input.metadata()?.is_file() {
        bail!("image path is not a file");
    }
    let mut snapshot = tempfile::Builder::new()
        .prefix("sai-image-display-")
        .suffix(".png")
        .tempfile()?;
    let length = std::io::copy(
        &mut Read::by_ref(&mut input).take(64 * 1024 * 1024 + 1),
        &mut snapshot,
    )?;
    if length > 64 * 1024 * 1024 {
        bail!("plugin image file exceeds byte limit");
    }
    snapshot.flush()?;
    let (width, height) = image::ImageReader::open(snapshot.path())?
        .with_guessed_format()?
        .into_dimensions()
        .context("invalid plugin image file")?;
    if width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || u64::from(width) * u64::from(height) > 32 * 1024 * 1024
    {
        bail!("plugin image dimensions exceed limit");
    }
    terminal::render(snapshot.path(), size)
}

/// 【插件图片】【尺寸上限】宽高必须落在终端的硬上限内，无效文本回退到默认尺寸。
/// @param size 可选 WIDTHxHEIGHT 文本，允许缺少一边
/// @returns 规范的有界尺寸
fn bounded_size(size: Option<&str>) -> Option<String> {
    let (width, height) = size?.trim().split_once('x')?;
    let width = width
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value.min(300).to_string())
        .unwrap_or_default();
    let height = height
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value.min(200).to_string())
        .unwrap_or_default();
    (!width.is_empty() || !height.is_empty()).then(|| format!("{width}x{height}"))
}

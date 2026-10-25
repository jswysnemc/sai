use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

const MAX_COLUMNS: usize = 24;
const MAX_ROWS: usize = 8;
const CACHE_ENTRIES: usize = 32;
const CACHE_BYTES: usize = 8 * 1024 * 1024;
type PreviewCache = VecDeque<(u64, String)>;

/// 【终端图片】【附件预览】生成固定上限的左对齐预览，缓存协议以稳定放置标识。
/// 参数: `source` 为文件路径或图片数据 URL，`width` 为可用列数；返回: 预览或语义化错误提示。
pub(crate) fn render(source: &str, width: usize) -> String {
    render_cached(source, width).unwrap_or_else(|_| {
        format!(
            "\x1b[2m{}\x1b[0m",
            crate::render::terminal_text("Image preview unavailable.", "图片预览不可用。")
        )
    })
}

/// 【终端图片】【预览缓存】文件内容变化、窗口尺寸或协议能力变化时重新渲染。
/// 参数: `source` 为图片来源，`width` 为可用列数；返回: 完整有界协议或读取错误。
fn render_cached(source: &str, width: usize) -> Result<String> {
    let columns = width.clamp(1, MAX_COLUMNS);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.hash(&mut hasher);
    width.hash(&mut hasher);
    crate::render::terminal_image::preview_environment().hash(&mut hasher);
    if !source.starts_with("data:") {
        let metadata = std::fs::metadata(source)?;
        metadata.len().hash(&mut hasher);
        metadata.modified().ok().hash(&mut hasher);
    }
    let key = hasher.finish();
    if let Some((_, rendered)) = cache().lock().unwrap().iter().find(|(id, _)| *id == key) {
        return Ok(rendered.clone());
    }
    // 1. 【终端图片】【来源解析】数据 URL 临时落盘，复用现有 Kitty 和降级实现
    let size = format!("{columns}x{MAX_ROWS}");
    let rendered = if source.starts_with("data:") {
        let (header, payload) = source.split_once(',').context("missing image payload")?;
        anyhow::ensure!(
            header.starts_with("data:image/") && header.ends_with(";base64"),
            "invalid image data URL"
        );
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(&STANDARD.decode(payload)?)?;
        crate::render::terminal_image::render_buffered_image(file.path(), Some(&size))?
    } else {
        crate::render::terminal_image::render_buffered_image(Path::new(source), Some(&size))?
    };
    let rendered = align_left(&rendered);
    // 2. 【终端图片】【缓存上限】按条目数与总字节数淘汰，避免图片持续占用内存
    if rendered.len() <= CACHE_BYTES {
        let mut cache = cache().lock().unwrap();
        while cache.len() >= CACHE_ENTRIES
            || cache.iter().map(|(_, text)| text.len()).sum::<usize>() + rendered.len()
                > CACHE_BYTES
        {
            cache.pop_front();
        }
        cache.push_back((key, rendered.clone()));
    }
    Ok(rendered)
}

/// 【终端图片】【左侧布局】不添加横向占位，只保留协议声明的完整图像高度。
/// 参数: `rendered` 为图片协议；返回: 左对齐预览及全部占位行。
fn align_left(rendered: &str) -> String {
    let mut lines = rendered
        .strip_suffix('\n')
        .unwrap_or(rendered)
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>();
    let reserved =
        crate::render::terminal_image::reserved_image_rows(rendered).unwrap_or(lines.len());
    lines.resize(reserved.max(lines.len()), String::new());
    lines.join("\n")
}

/// 【终端图片】【缓存状态】读取进程内有界缓存。
/// 参数: 无；返回: 预览缓存锁。
fn cache() -> &'static Mutex<PreviewCache> {
    static CACHE: OnceLock<Mutex<PreviewCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
#[path = "image_preview_tests.rs"]
mod tests;

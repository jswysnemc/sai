use anyhow::{Context, Result};
use base64::Engine;
use std::path::{Path, PathBuf};

pub(crate) struct MediaBytes {
    pub(crate) path: PathBuf,
    pub(crate) filename: String,
    pub(crate) bytes: Vec<u8>,
}

impl MediaBytes {
    /// 读取本地媒体文件。
    ///
    /// 参数:
    /// - `path`: 本地文件路径
    ///
    /// 返回:
    /// - 文件名和字节内容
    pub(crate) fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read media file: {}", path.display()))?;
        let filename = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "file".to_string());
        Ok(Self {
            path: path.to_path_buf(),
            filename,
            bytes,
        })
    }

    /// 返回文件内容的 base64 编码。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - base64 字符串
    pub(crate) fn base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.bytes)
    }

    /// 返回文件内容的 md5 摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 小写十六进制 md5
    pub(crate) fn md5_hex(&self) -> String {
        format!("{:x}", md5::compute(&self.bytes))
    }

    /// 推断文件 MIME 类型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - MIME 类型字符串
    pub(crate) fn content_type(&self) -> &'static str {
        match self
            .path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "pdf" => "application/pdf",
            "txt" | "md" | "log" => "text/plain",
            "json" => "application/json",
            "mp4" => "video/mp4",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            _ => "application/octet-stream",
        }
    }
}

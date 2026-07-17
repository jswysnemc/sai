use super::event::{QqBotInboundMedia, QqBotInboundMediaKind};
use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use std::path::{Path, PathBuf};

const MAX_INBOUND_MEDIA_BYTES: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct SavedQqInboundMedia {
    pub(crate) kind: QqBotInboundMediaKind,
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) mime_type: String,
}

/// 下载并保存 QQ 入站媒体。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `http_client`: HTTP 客户端
/// - `media`: 入站媒体元数据
///
/// 返回:
/// - 已保存媒体文件信息
pub(crate) async fn save_inbound_media(
    paths: &crate::paths::SaiPaths,
    http_client: &reqwest::Client,
    media: &QqBotInboundMedia,
) -> Result<SavedQqInboundMedia> {
    let (bytes, source_mime) = read_media_bytes(http_client, &media.source).await?;
    if bytes.len() > MAX_INBOUND_MEDIA_BYTES {
        bail!(
            "{} {} bytes",
            t("QQ attachment exceeds", "QQ 附件超过"),
            MAX_INBOUND_MEDIA_BYTES
        );
    }
    let mime_type = infer_mime_type(media, &bytes, source_mime.as_deref());
    let name = media
        .name
        .clone()
        .unwrap_or_else(|| default_file_name(media.kind, &mime_type));
    let path = save_bytes(paths, &name, &bytes)?;
    Ok(SavedQqInboundMedia {
        kind: media.kind,
        path,
        name,
        mime_type,
    })
}

/// 将已保存 QQ 图片转换为 data URL。
///
/// 参数:
/// - `saved`: 已保存媒体信息
///
/// 返回:
/// - 图片 data URL
pub(crate) fn saved_image_to_data_url(saved: &SavedQqInboundMedia) -> Result<String> {
    let bytes = std::fs::read(&saved.path).with_context(|| {
        format!(
            "{}: {}",
            t("failed to read saved QQ image", "读取已保存 QQ 图片失败"),
            saved.path.display()
        )
    })?;
    let content_type = image_mime(&bytes).ok_or_else(|| {
        anyhow::anyhow!(t(
            "the saved QQ image format is not supported by the model",
            "已保存 QQ 图片不是模型支持的图片格式"
        ))
    })?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{content_type};base64,{encoded}"))
}

/// 读取 QQ 入站媒体字节。
///
/// 参数:
/// - `http_client`: HTTP 客户端
/// - `source`: 媒体来源
///
/// 返回:
/// - 媒体字节和可选 MIME 类型
async fn read_media_bytes(
    http_client: &reqwest::Client,
    source: &str,
) -> Result<(Vec<u8>, Option<String>)> {
    if source.starts_with("data:") {
        return read_data_url(source);
    }
    if is_http_url(source) {
        return download_bytes(http_client, source).await;
    }
    let path = local_source_path(source);
    let bytes = std::fs::read(&path).with_context(|| {
        format!(
            "{}: {}",
            t("failed to read QQ inbound file", "读取 QQ 入站文件失败"),
            path.display()
        )
    })?;
    Ok((bytes, mime_from_path(&path)))
}

/// 下载 HTTP 媒体字节。
///
/// 参数:
/// - `http_client`: HTTP 客户端
/// - `url`: 媒体 URL
///
/// 返回:
/// - 媒体字节和可选 MIME 类型
async fn download_bytes(
    http_client: &reqwest::Client,
    url: &str,
) -> Result<(Vec<u8>, Option<String>)> {
    let response = http_client.get(url).send().await.with_context(|| {
        format!(
            "{}: {url}",
            t(
                "QQ attachment download request failed",
                "QQ 附件下载请求失败"
            )
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        bail!(
            "{} HTTP {status}",
            t("QQ attachment download failed", "QQ 附件下载失败")
        );
    }
    if let Some(len) = response.content_length() {
        if len > MAX_INBOUND_MEDIA_BYTES as u64 {
            bail!(
                "{} {} bytes",
                t("QQ attachment exceeds", "QQ 附件超过"),
                MAX_INBOUND_MEDIA_BYTES
            );
        }
    }
    let mime_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes = response.bytes().await?.to_vec();
    Ok((bytes, mime_type))
}

/// 读取 data URL 字节。
///
/// 参数:
/// - `source`: data URL 文本
///
/// 返回:
/// - 媒体字节和可选 MIME 类型
fn read_data_url(source: &str) -> Result<(Vec<u8>, Option<String>)> {
    let Some((prefix, encoded)) = source.split_once(',') else {
        bail!(t("invalid QQ data URL format", "QQ data URL 格式无效"));
    };
    if !prefix.contains(";base64") {
        bail!(t(
            "QQ data URL only supports base64 encoding",
            "QQ data URL 只支持 base64 编码"
        ));
    }
    let mime_type = prefix
        .strip_prefix("data:")
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .with_context(|| {
            t(
                "QQ data URL base64 decoding failed",
                "QQ data URL base64 解码失败",
            )
        })?;
    Ok((bytes, mime_type))
}

/// 推断媒体 MIME 类型。
///
/// 参数:
/// - `media`: 入站媒体元数据
/// - `bytes`: 媒体字节
/// - `source_mime`: 来源提供的 MIME 类型
///
/// 返回:
/// - MIME 类型
fn infer_mime_type(media: &QqBotInboundMedia, bytes: &[u8], source_mime: Option<&str>) -> String {
    if let Some(mime) = image_mime(bytes) {
        return mime.to_string();
    }
    if let Some(mime) = source_mime.map(str::trim).filter(|value| !value.is_empty()) {
        return mime.to_string();
    }
    if let Some(name) = &media.name {
        let mime = mime_guess::from_path(name).first_or_octet_stream();
        return mime.essence_str().to_string();
    }
    match media.kind {
        QqBotInboundMediaKind::Image => "image/*",
        QqBotInboundMediaKind::Voice => "audio/*",
        QqBotInboundMediaKind::Video => "video/*",
        QqBotInboundMediaKind::File => "application/octet-stream",
    }
    .to_string()
}

/// 根据图片魔数推断 MIME 类型。
///
/// 参数:
/// - `bytes`: 图片字节
///
/// 返回:
/// - 图片 MIME 类型
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// 生成默认文件名。
///
/// 参数:
/// - `kind`: 媒体类型
/// - `mime_type`: MIME 类型
///
/// 返回:
/// - 默认文件名
fn default_file_name(kind: QqBotInboundMediaKind, mime_type: &str) -> String {
    let stem = match kind {
        QqBotInboundMediaKind::Image => "image",
        QqBotInboundMediaKind::Voice => "voice",
        QqBotInboundMediaKind::Video => "video",
        QqBotInboundMediaKind::File => "file",
    };
    let extension = match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        _ => "bin",
    };
    format!("{stem}.{extension}")
}

/// 保存媒体字节到 QQ 入站目录。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `name`: 原始文件名
/// - `bytes`: 媒体字节
///
/// 返回:
/// - 保存后的文件路径
fn save_bytes(paths: &crate::paths::SaiPaths, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    let dir = paths
        .state_dir
        .join("gateways")
        .join("qq")
        .join("inbound")
        .join(chrono::Local::now().format("%Y%m%d").to_string());
    std::fs::create_dir_all(&dir)?;
    let safe_name = sanitize_file_name(name);
    let path = dir.join(format!(
        "{}-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u16>(),
        safe_name
    ));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// 判断字符串是否为 HTTP URL。
///
/// 参数:
/// - `source`: 资源地址
///
/// 返回:
/// - 是否为 HTTP URL
fn is_http_url(source: &str) -> bool {
    source.starts_with("http://") || source.starts_with("https://")
}

/// 将本地资源地址转换为文件路径。
///
/// 参数:
/// - `source`: 资源地址
///
/// 返回:
/// - 本地文件路径
fn local_source_path(source: &str) -> PathBuf {
    source
        .strip_prefix("file://")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(source))
}

/// 根据路径推断 MIME 类型。
///
/// 参数:
/// - `path`: 本地路径
///
/// 返回:
/// - MIME 类型
fn mime_from_path(path: &Path) -> Option<String> {
    mime_guess::from_path(path)
        .first()
        .map(|mime| mime.essence_str().to_string())
}

/// 清理文件名。
///
/// 参数:
/// - `name`: 原始文件名
///
/// 返回:
/// - 安全文件名
fn sanitize_file_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches([' ', '.']).trim();
    if trimmed.is_empty() {
        "file.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize_file_name("a/b:c.pdf"), "a_b_c.pdf");
        assert_eq!(sanitize_file_name(".."), "file.bin");
    }

    #[test]
    fn reads_base64_data_url() {
        let (bytes, mime) = read_data_url("data:text/plain;base64,aGVsbG8=").unwrap();

        assert_eq!(bytes, b"hello");
        assert_eq!(mime.as_deref(), Some("text/plain"));
    }
}

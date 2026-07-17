use super::client::WeixinBotClient;
use super::event::{WeixinInboundMedia, WeixinInboundMediaKind};
use crate::i18n::text as t;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyInit};
use anyhow::{bail, Context, Result};
use base64::Engine;
use std::path::PathBuf;

const MAX_INBOUND_MEDIA_BYTES: usize = 100 * 1024 * 1024;

type Aes128EcbDecryptor = ecb::Decryptor<aes::Aes128>;

#[derive(Debug, Clone)]
pub(crate) struct SavedInboundMedia {
    pub(crate) kind: WeixinInboundMediaKind,
    pub(crate) path: PathBuf,
    pub(crate) name: String,
    pub(crate) mime_type: String,
}

/// 下载、解密并保存微信入站媒体。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `client`: 微信 iLink 客户端
/// - `http_client`: HTTP 客户端
/// - `media`: 入站媒体元数据
///
/// 返回:
/// - 已保存媒体文件信息
pub(crate) async fn save_inbound_media(
    paths: &crate::paths::SaiPaths,
    client: &WeixinBotClient,
    http_client: &reqwest::Client,
    media: &WeixinInboundMedia,
) -> Result<SavedInboundMedia> {
    let download = media.download.as_ref().ok_or_else(|| {
        anyhow::anyhow!(t(
            "Weixin attachment has no download information",
            "微信附件缺少可下载信息"
        ))
    })?;
    let url = resolve_download_url(client, download)?;
    client.debug_log(format!(
        "{} kind={} source={} url={}",
        t("inbound attachment download started", "入站附件下载开始"),
        inbound_kind_name(media.kind),
        media.source,
        redact_url_for_log(&url)
    ));
    let mut bytes = download_bytes(http_client, &url).await?;
    client.debug_log(format!(
        "{} kind={} encrypted_or_plain_bytes={}",
        t("inbound attachment download completed", "入站附件下载完成"),
        inbound_kind_name(media.kind),
        bytes.len()
    ));
    if let Some(aes_key) = download.aes_key.as_deref() {
        let key = parse_aes_key(aes_key)?;
        bytes = decrypt_aes_ecb(&bytes, &key)?;
        client.debug_log(format!(
            "{} kind={} plain_bytes={}",
            t(
                "inbound attachment decryption completed",
                "入站附件解密完成"
            ),
            inbound_kind_name(media.kind),
            bytes.len()
        ));
    }
    if bytes.len() > MAX_INBOUND_MEDIA_BYTES {
        bail!(
            "{} {} bytes",
            t("Weixin attachment exceeds", "微信附件超过"),
            MAX_INBOUND_MEDIA_BYTES
        );
    }
    let mime_type = infer_mime_type(media, &bytes);
    let name = media
        .name
        .clone()
        .unwrap_or_else(|| default_file_name(media.kind, &mime_type));
    let path = save_bytes(paths, &name, &bytes)?;
    client.debug_log(format!(
        "{} kind={} name={} mime={} path={}",
        t("inbound attachment saved", "入站附件已保存"),
        inbound_kind_name(media.kind),
        name,
        mime_type,
        path.display()
    ));
    Ok(SavedInboundMedia {
        kind: media.kind,
        path,
        name,
        mime_type,
    })
}

/// 解析微信 CDN 下载地址。
///
/// 参数:
/// - `client`: 微信 iLink 客户端
/// - `download`: 媒体下载元数据
///
/// 返回:
/// - 可请求的下载 URL
fn resolve_download_url(
    client: &WeixinBotClient,
    download: &super::event::WeixinInboundMediaDownload,
) -> Result<String> {
    if let Some(full_url) = download
        .full_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(full_url.to_string());
    }
    let param = download
        .encrypt_query_param
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(t(
                "Weixin attachment has no full_url or encrypt_query_param",
                "微信附件缺少 full_url 或 encrypt_query_param"
            ))
        })?;
    Ok(format!(
        "{}/download?encrypted_query_param={}",
        client.cdn_base_url().trim_end_matches('/'),
        urlencoding::encode(param)
    ))
}

/// 下载媒体字节。
///
/// 参数:
/// - `client`: HTTP 客户端
/// - `url`: 下载 URL
///
/// 返回:
/// - 下载后的字节
async fn download_bytes(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client.get(url).send().await.with_context(|| {
        t(
            "Weixin attachment download request failed",
            "微信附件下载请求失败",
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        bail!(
            "{} HTTP {status}",
            t("Weixin attachment download failed", "微信附件下载失败")
        );
    }
    if let Some(len) = response.content_length() {
        if len > MAX_INBOUND_MEDIA_BYTES as u64 {
            bail!(
                "{} {} bytes",
                t("Weixin attachment exceeds", "微信附件超过"),
                MAX_INBOUND_MEDIA_BYTES
            );
        }
    }
    let bytes = response.bytes().await?.to_vec();
    if bytes.len() > MAX_INBOUND_MEDIA_BYTES {
        bail!(
            "{} {} bytes",
            t("Weixin attachment exceeds", "微信附件超过"),
            MAX_INBOUND_MEDIA_BYTES
        );
    }
    Ok(bytes)
}

/// 解析微信 CDN AES key。
///
/// 参数:
/// - `aes_key`: base64 编码的 AES key
///
/// 返回:
/// - 16 字节 AES key
fn parse_aes_key(aes_key: &str) -> Result<[u8; 16]> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(aes_key)
        .with_context(|| {
            t(
                "Weixin attachment AES key is not valid base64",
                "微信附件 AES key 不是有效 base64",
            )
        })?;
    let raw = if decoded.len() == 16 {
        decoded
    } else if decoded.len() == 32 && decoded.iter().all(u8::is_ascii_hexdigit) {
        hex::decode(std::str::from_utf8(&decoded)?).with_context(|| {
            t(
                "Weixin attachment AES key is not valid hex",
                "微信附件 AES key 不是有效 hex",
            )
        })?
    } else {
        bail!(
            "{}: {} bytes",
            t(
                "invalid Weixin attachment AES key length",
                "微信附件 AES key 长度无效"
            ),
            decoded.len()
        );
    };
    raw.try_into().map_err(|value: Vec<u8>| {
        anyhow::anyhow!(
            "{}: {} bytes",
            t(
                "invalid Weixin attachment AES key length",
                "微信附件 AES key 长度无效"
            ),
            value.len()
        )
    })
}

/// 使用 AES-128-ECB PKCS7 解密媒体字节。
///
/// 参数:
/// - `bytes`: 密文字节
/// - `key`: 16 字节 AES key
///
/// 返回:
/// - 明文字节
fn decrypt_aes_ecb(bytes: &[u8], key: &[u8; 16]) -> Result<Vec<u8>> {
    Aes128EcbDecryptor::new(key.into())
        .decrypt_padded_vec_mut::<Pkcs7>(bytes)
        .map_err(|err| {
            anyhow::anyhow!(
                "{}: {err}",
                t(
                    "Weixin attachment AES decryption failed",
                    "微信附件 AES 解密失败"
                )
            )
        })
}

/// 推断媒体 MIME 类型。
///
/// 参数:
/// - `media`: 入站媒体元数据
/// - `bytes`: 媒体明文字节
///
/// 返回:
/// - MIME 类型
fn infer_mime_type(media: &WeixinInboundMedia, bytes: &[u8]) -> String {
    if let Some(mime) = image_mime(bytes) {
        return mime.to_string();
    }
    if let Some(name) = &media.name {
        let mime = mime_guess::from_path(name).first_or_octet_stream();
        return mime.essence_str().to_string();
    }
    match media.kind {
        WeixinInboundMediaKind::Image => "image/*",
        WeixinInboundMediaKind::Voice => "audio/silk",
        WeixinInboundMediaKind::Video => "video/mp4",
        WeixinInboundMediaKind::File => "application/octet-stream",
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
pub(crate) fn image_mime(bytes: &[u8]) -> Option<&'static str> {
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
fn default_file_name(kind: WeixinInboundMediaKind, mime_type: &str) -> String {
    let stem = match kind {
        WeixinInboundMediaKind::Image => "image",
        WeixinInboundMediaKind::Voice => "voice",
        WeixinInboundMediaKind::Video => "video",
        WeixinInboundMediaKind::File => "file",
    };
    let extension = match mime_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "audio/silk" => "silk",
        _ => "bin",
    };
    format!("{stem}.{extension}")
}

/// 保存媒体字节到微信入站目录。
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
        .join("weixin")
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

/// 返回入站媒体类型名称。
///
/// 参数:
/// - `kind`: 入站媒体类型
///
/// 返回:
/// - 媒体类型文本
fn inbound_kind_name(kind: WeixinInboundMediaKind) -> &'static str {
    match kind {
        WeixinInboundMediaKind::Image => "image",
        WeixinInboundMediaKind::Voice => "voice",
        WeixinInboundMediaKind::Video => "video",
        WeixinInboundMediaKind::File => "file",
    }
}

/// 脱敏 URL 日志。
///
/// 参数:
/// - `url`: 原始 URL
///
/// 返回:
/// - 不含查询参数的 URL
fn redact_url_for_log(url: &str) -> String {
    url.split('?').next().unwrap_or(url).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_raw_and_hex_aes_keys() {
        let raw_key = [7u8; 16];
        let raw = base64::engine::general_purpose::STANDARD.encode(raw_key);
        assert_eq!(parse_aes_key(&raw).unwrap(), raw_key);

        let hex_text = hex::encode(raw_key);
        let hex_base64 = base64::engine::general_purpose::STANDARD.encode(hex_text);
        assert_eq!(parse_aes_key(&hex_base64).unwrap(), raw_key);
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize_file_name("a/b:c.pdf"), "a_b_c.pdf");
        assert_eq!(sanitize_file_name(".."), "file.bin");
    }
}

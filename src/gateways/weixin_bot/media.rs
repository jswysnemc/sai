use super::client::WeixinBotClient;
use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyInit};
use anyhow::{bail, Context, Result};
use base64::Engine;
use rand::RngCore;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MAX_OUTBOUND_MEDIA_BYTES: u64 = 100 * 1024 * 1024;
const UPLOAD_MEDIA_IMAGE: i64 = 1;
const UPLOAD_MEDIA_VIDEO: i64 = 2;
const UPLOAD_MEDIA_FILE: i64 = 3;
const CDN_UPLOAD_RETRIES: usize = 3;

type Aes128EcbEncryptor = ecb::Encryptor<aes::Aes128>;

#[derive(Debug, Clone, Copy)]
pub(crate) enum WeixinOutboundMediaKind {
    Image,
    Video,
    File,
}

#[derive(Debug, Clone)]
struct UploadedMedia {
    download_param: String,
    aes_key: Vec<u8>,
    raw_size: usize,
    cipher_size: usize,
}

/// 发送微信本地媒体文件。
///
/// 参数:
/// - `client`: 微信 iLink 客户端
/// - `to_user_id`: 接收方微信 iLink 用户 ID
/// - `context_token`: 入站消息上下文 token
/// - `path`: 本地文件路径
/// - `caption`: 可选说明文本
/// - `kind`: 媒体类型
///
/// 返回:
/// - 发送成功的消息 ID
pub(crate) async fn send_local_media(
    client: &WeixinBotClient,
    to_user_id: &str,
    context_token: Option<&str>,
    path: &Path,
    caption: Option<&str>,
    kind: WeixinOutboundMediaKind,
) -> Result<String> {
    let file_path = validate_media_file(path, kind)?;
    let bytes = std::fs::read(&file_path)
        .with_context(|| format!("读取媒体文件失败: {}", file_path.display()))?;
    let file_name = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("file")
        .to_string();
    client.debug_log(format!(
        "准备发送本地媒体 kind={} path={} bytes={}",
        media_kind_name(kind),
        file_path.display(),
        bytes.len()
    ));
    let uploaded = upload_media(client, to_user_id, &bytes, kind).await?;
    if let Some(text) = caption.filter(|value| !value.trim().is_empty()) {
        client.send_text(to_user_id, text, context_token).await?;
    }
    let item = media_message_item(kind, &file_name, &uploaded);
    client
        .send_message_item(to_user_id, item, context_token)
        .await
}

/// 校验本地媒体文件路径和类型。
///
/// 参数:
/// - `path`: 待发送文件路径
/// - `kind`: 媒体类型
///
/// 返回:
/// - 规范化后的文件路径
fn validate_media_file(path: &Path, kind: WeixinOutboundMediaKind) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()?.join(path)
    };
    let metadata =
        std::fs::metadata(&path).with_context(|| format!("媒体文件不存在: {}", path.display()))?;
    if !metadata.is_file() {
        bail!("媒体路径不是文件: {}", path.display());
    }
    if metadata.len() > MAX_OUTBOUND_MEDIA_BYTES {
        bail!(
            "媒体文件超过 {} bytes: {}",
            MAX_OUTBOUND_MEDIA_BYTES,
            path.display()
        );
    }
    let mime = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    match kind {
        WeixinOutboundMediaKind::Image if !mime.starts_with("image/") => {
            bail!("send_channel_image 只能发送图片文件: {}", path.display());
        }
        WeixinOutboundMediaKind::Video if !mime.starts_with("video/") => {
            bail!("send_channel_video 只能发送视频文件: {}", path.display());
        }
        WeixinOutboundMediaKind::File => {}
        WeixinOutboundMediaKind::Image | WeixinOutboundMediaKind::Video => {}
    }
    Ok(path)
}

/// 上传媒体文件到微信 CDN。
///
/// 参数:
/// - `client`: 微信 iLink 客户端
/// - `to_user_id`: 接收方微信 iLink 用户 ID
/// - `bytes`: 明文文件内容
/// - `kind`: 媒体类型
///
/// 返回:
/// - 上传后的媒体引用信息
async fn upload_media(
    client: &WeixinBotClient,
    to_user_id: &str,
    bytes: &[u8],
    kind: WeixinOutboundMediaKind,
) -> Result<UploadedMedia> {
    let mut aes_key = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut aes_key);
    let mut file_key_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut file_key_bytes);
    let file_key = hex::encode(file_key_bytes);
    let raw_size = bytes.len();
    let cipher_size = aes_ecb_padded_size(raw_size);
    let raw_md5 = format!("{:x}", md5::compute(bytes));
    client.debug_log(format!(
        "请求 CDN 上传地址 kind={} raw_bytes={} cipher_bytes={cipher_size}",
        media_kind_name(kind),
        raw_size
    ));
    let upload_url = client
        .get_upload_url(json!({
            "filekey": file_key,
            "media_type": upload_media_type(kind),
            "to_user_id": to_user_id,
            "rawsize": raw_size,
            "rawfilemd5": raw_md5,
            "filesize": cipher_size,
            "no_need_thumb": true,
            "aeskey": hex::encode(aes_key),
        }))
        .await?;
    let encrypted = encrypt_aes_ecb(bytes, &aes_key);
    let download_param = upload_encrypted_bytes(client, &upload_url, &file_key, &encrypted).await?;
    client.debug_log(format!(
        "CDN 上传完成 kind={} encrypted_bytes={} download_param_present={}",
        media_kind_name(kind),
        encrypted.len(),
        !download_param.trim().is_empty()
    ));
    Ok(UploadedMedia {
        download_param,
        aes_key: aes_key.to_vec(),
        raw_size,
        cipher_size: encrypted.len(),
    })
}

/// 返回微信上传媒体类型值。
///
/// 参数:
/// - `kind`: 媒体类型
///
/// 返回:
/// - 微信 UploadMediaType 数值
fn upload_media_type(kind: WeixinOutboundMediaKind) -> i64 {
    match kind {
        WeixinOutboundMediaKind::Image => UPLOAD_MEDIA_IMAGE,
        WeixinOutboundMediaKind::Video => UPLOAD_MEDIA_VIDEO,
        WeixinOutboundMediaKind::File => UPLOAD_MEDIA_FILE,
    }
}

/// 计算 AES-128-ECB PKCS7 加密后的字节数。
///
/// 参数:
/// - `raw_size`: 明文字节数
///
/// 返回:
/// - 加密后字节数
fn aes_ecb_padded_size(raw_size: usize) -> usize {
    ((raw_size + 1 + 15) / 16) * 16
}

/// 使用 AES-128-ECB PKCS7 加密明文。
///
/// 参数:
/// - `bytes`: 明文字节
/// - `key`: 16 字节 AES 密钥
///
/// 返回:
/// - 密文字节
fn encrypt_aes_ecb(bytes: &[u8], key: &[u8; 16]) -> Vec<u8> {
    Aes128EcbEncryptor::new(key.into()).encrypt_padded_vec_mut::<Pkcs7>(bytes)
}

/// 上传加密后的媒体字节到微信 CDN。
///
/// 参数:
/// - `client`: 微信 iLink 客户端
/// - `upload_url`: getuploadurl 响应 JSON
/// - `file_key`: 文件 key
/// - `encrypted`: 加密后的媒体字节
///
/// 返回:
/// - CDN 返回的下载加密参数
async fn upload_encrypted_bytes(
    client: &WeixinBotClient,
    upload_url: &Value,
    file_key: &str,
    encrypted: &[u8],
) -> Result<String> {
    let url = resolve_upload_url(client, upload_url, file_key)?;
    client.debug_log(format!(
        "CDN 上传地址 host_path={}",
        redact_url_for_log(&url)
    ));
    let http = reqwest::Client::new();
    let mut last_error = None;
    for attempt in 1..=CDN_UPLOAD_RETRIES {
        client.debug_log(format!(
            "CDN 上传开始 attempt={attempt} encrypted_bytes={}",
            encrypted.len()
        ));
        let response = http
            .post(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(encrypted.to_vec())
            .send()
            .await;
        match response {
            Ok(response) if response.status().as_u16() == 200 => {
                client.debug_log(format!("CDN 上传 HTTP 200 attempt={attempt}"));
                return response
                    .headers()
                    .get("x-encrypted-param")
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned)
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("微信 CDN 上传响应缺少 x-encrypted-param"));
            }
            Ok(response) if response.status().is_client_error() => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                client.debug_log(format!(
                    "CDN 上传客户端错误 attempt={attempt} status={status} body={}",
                    truncate_for_log(&body)
                ));
                bail!("微信 CDN 上传失败 HTTP {status}: {body}");
            }
            Ok(response) => {
                client.debug_log(format!(
                    "CDN 上传服务端错误 attempt={attempt} status={}",
                    response.status()
                ));
                last_error = Some(anyhow::anyhow!(
                    "微信 CDN 上传失败 HTTP {}",
                    response.status()
                ));
            }
            Err(err) => {
                client.debug_log(format!("CDN 上传请求错误 attempt={attempt}: {err}"));
                last_error = Some(anyhow::anyhow!("微信 CDN 上传请求失败: {err}"));
            }
        }
        if attempt < CDN_UPLOAD_RETRIES {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("微信 CDN 上传失败")))
}

/// 解析微信 CDN 上传地址。
///
/// 参数:
/// - `client`: 微信 iLink 客户端
/// - `upload_url`: getuploadurl 响应 JSON
/// - `file_key`: 文件 key
///
/// 返回:
/// - 可直接上传的 CDN 地址
fn resolve_upload_url(
    client: &WeixinBotClient,
    upload_url: &Value,
    file_key: &str,
) -> Result<String> {
    if let Some(full_url) = upload_url
        .get("upload_full_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(full_url.to_string());
    }
    let upload_param = upload_url
        .get("upload_param")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("微信 getuploadurl 响应缺少 upload_full_url 或 upload_param")
        })?;
    Ok(format!(
        "{}/upload?encrypted_query_param={}&filekey={}",
        client.cdn_base_url().trim_end_matches('/'),
        urlencoding::encode(upload_param),
        urlencoding::encode(file_key)
    ))
}

/// 构建微信媒体消息项。
///
/// 参数:
/// - `kind`: 媒体类型
/// - `file_name`: 文件名
/// - `uploaded`: 上传后的媒体引用信息
///
/// 返回:
/// - MessageItem JSON
fn media_message_item(
    kind: WeixinOutboundMediaKind,
    file_name: &str,
    uploaded: &UploadedMedia,
) -> Value {
    let media = json!({
        "encrypt_query_param": uploaded.download_param,
        "aes_key": weixin_media_aes_key(&uploaded.aes_key),
        "encrypt_type": 1,
    });
    match kind {
        WeixinOutboundMediaKind::Image => json!({
            "type": 2,
            "image_item": {
                "media": media,
                "mid_size": uploaded.cipher_size,
            }
        }),
        WeixinOutboundMediaKind::Video => json!({
            "type": 5,
            "video_item": {
                "media": media,
                "video_size": uploaded.cipher_size,
            }
        }),
        WeixinOutboundMediaKind::File => json!({
            "type": 4,
            "file_item": {
                "media": media,
                "file_name": file_name,
                "len": uploaded.raw_size.to_string(),
            }
        }),
    }
}

/// 构建微信媒体消息中的 aes_key 字段。
///
/// 参数:
/// - `aes_key`: 原始 16 字节 AES key
///
/// 返回:
/// - base64(hex(raw key)) 编码结果
fn weixin_media_aes_key(aes_key: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(hex::encode(aes_key))
}

/// 返回媒体类型名称。
///
/// 参数:
/// - `kind`: 媒体类型
///
/// 返回:
/// - 媒体类型文本
fn media_kind_name(kind: WeixinOutboundMediaKind) -> &'static str {
    match kind {
        WeixinOutboundMediaKind::Image => "image",
        WeixinOutboundMediaKind::Video => "video",
        WeixinOutboundMediaKind::File => "file",
    }
}

/// 截断日志文本。
///
/// 参数:
/// - `text`: 原始文本
///
/// 返回:
/// - 截断后的文本
fn truncate_for_log(text: &str) -> String {
    const LIMIT: usize = 500;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let clipped = text.chars().take(LIMIT).collect::<String>();
    format!("{clipped}...[truncated]")
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
    fn aes_padding_size_matches_pkcs7_blocks() {
        assert_eq!(aes_ecb_padded_size(0), 16);
        assert_eq!(aes_ecb_padded_size(1), 16);
        assert_eq!(aes_ecb_padded_size(15), 16);
        assert_eq!(aes_ecb_padded_size(16), 32);
    }

    #[test]
    fn media_aes_key_matches_weixin_hex_base64_format() {
        let key = [0x11u8; 16];
        let encoded = weixin_media_aes_key(&key);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();

        assert_eq!(std::str::from_utf8(&decoded).unwrap(), hex::encode(key));
    }
}

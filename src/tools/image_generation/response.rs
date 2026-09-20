use anyhow::{bail, Context, Result};
use base64::Engine;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;

/// 供应商返回的一张待保存图片。
#[derive(Debug, Clone)]
pub(super) struct ImagePayload {
    pub(super) bytes: Vec<u8>,
    pub(super) mime_hint: Option<String>,
    pub(super) source: &'static str,
}

#[derive(Debug, Clone)]
enum Candidate {
    Bytes {
        bytes: Vec<u8>,
        mime_hint: Option<String>,
        source: &'static str,
    },
    Url(String),
}

/// 兼容图片接口常见的 JSON、原始图片、Base64、URL 和 HTML 图片块。
///
/// 参数:
/// - body: HTTP 响应正文
/// - content_type: 响应媒体类型
/// - client: 用于匿名下载供应商返回的图片 URL
///
/// 返回:
/// - 已解码的图片字节；不把供应商返回的大字段原样传给调用方
pub(super) async fn decode_response(
    body: Vec<u8>,
    content_type: Option<&str>,
    client: Arc<reqwest::Client>,
) -> Result<Vec<ImagePayload>> {
    if body.len() > MAX_IMAGE_BYTES {
        bail!("image response exceeds {} bytes", MAX_IMAGE_BYTES);
    }
    let content_type = content_type
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut candidates = Vec::new();
    if content_type.is_some_and(|value| value.starts_with("image/")) {
        candidates.push(Candidate::Bytes {
            bytes: body,
            mime_hint: content_type.map(str::to_string),
            source: "image_response",
        });
    } else if let Ok(value) = serde_json::from_slice::<Value>(&body) {
        collect_json_candidates(&value, None, &mut candidates)?;
    } else {
        collect_text_candidate(
            std::str::from_utf8(&body).context("image response is not UTF-8")?,
            &mut candidates,
        )?;
    }
    let mut images = Vec::new();
    let mut seen = HashSet::new();
    for candidate in candidates {
        let image = match candidate {
            Candidate::Bytes {
                bytes,
                mime_hint,
                source,
            } => ImagePayload {
                bytes,
                mime_hint,
                source,
            },
            Candidate::Url(url) => download_image(&client, &url).await?,
        };
        let key = blake3::hash(&image.bytes).to_hex().to_string();
        if seen.insert(key) {
            images.push(image);
        }
    }
    if images.is_empty() {
        bail!("image endpoint returned no image data");
    }
    Ok(images)
}

/// 从 JSON 结构递归提取图片字段，兼容 OpenAI 风格 data 数组和供应商自定义字段。
///
/// 参数:
/// - value: 当前 JSON 节点
/// - key_hint: 父节点字段名
/// - out: 候选图片集合
///
/// 返回:
/// - 提取结果；没有可识别字段时保持为空
fn collect_json_candidates(
    value: &Value,
    key_hint: Option<&str>,
    out: &mut Vec<Candidate>,
) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                collect_json_candidates(value, Some(key), out)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_json_candidates(item, key_hint, out)?;
            }
        }
        Value::String(text) => {
            let key = key_hint.unwrap_or_default().to_ascii_lowercase();
            if key.is_empty() {
                collect_text_candidate(text, out)?;
            } else if key.contains("html") || key.contains("markup") {
                collect_html_candidates(text, out)?;
            } else if key.contains("url") || key == "uri" || key == "src" {
                if let Some(candidate) = url_candidate(text) {
                    out.push(candidate);
                } else if let Some(candidate) = data_url_candidate(text)? {
                    out.push(candidate);
                }
            } else if key.contains("base64") || key.contains("b64") {
                out.push(Candidate::Bytes {
                    bytes: decode_base64(text)?,
                    mime_hint: None,
                    source: "base64",
                });
            } else if key.contains("image")
                || key == "data"
                || key == "content"
                || key == "output"
                || key == "result"
            {
                collect_text_candidate(text, out)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// 解析文本形式的 data URL、URL、HTML 图片块或裸 Base64。
///
/// 参数:
/// - text: 供应商返回的文本
/// - out: 候选图片集合
///
/// 返回:
/// - 解析结果
fn collect_text_candidate(text: &str, out: &mut Vec<Candidate>) -> Result<()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    if text.starts_with('<') {
        return collect_html_candidates(text, out);
    }
    if let Some(candidate) = data_url_candidate(text)? {
        out.push(candidate);
        return Ok(());
    }
    if let Some(candidate) = url_candidate(text) {
        out.push(candidate);
        return Ok(());
    }
    if text.len() >= 16 {
        out.push(Candidate::Bytes {
            bytes: decode_base64(text)?,
            mime_hint: None,
            source: "base64",
        });
    }
    Ok(())
}

/// 提取 HTML 或 Markdown 中的图片地址，不执行 HTML。
///
/// 参数:
/// - text: HTML 或 Markdown 文本
/// - out: 候选图片集合
///
/// 返回:
/// - 解析结果
fn collect_html_candidates(text: &str, out: &mut Vec<Candidate>) -> Result<()> {
    let lower = text.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(start) = lower[offset..].find("<img") {
        let start = offset + start;
        let Some(end) = lower[start..].find('>') else {
            break;
        };
        let tag = &text[start..start + end];
        if let Some(src) = attribute_value(tag, "src") {
            if let Some(candidate) = data_url_candidate(&src)? {
                out.push(candidate);
            } else if let Some(candidate) = url_candidate(&src) {
                out.push(candidate);
            }
        }
        offset = start + end + 1;
    }
    let mut rest = text;
    while let Some(start) = rest.find("](") {
        let value = &rest[start + 2..];
        let Some(end) = value.find(')') else {
            break;
        };
        let url = value[..end].trim().trim_matches(['<', '>']);
        if let Some(candidate) = data_url_candidate(url)? {
            out.push(candidate);
        } else if let Some(candidate) = url_candidate(url) {
            out.push(candidate);
        }
        rest = &value[end + 1..];
    }
    Ok(())
}

/// 从 HTML 标签中读取双引号或单引号属性。
fn attribute_value(tag: &str, attribute: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let start = lower
        .match_indices(attribute)
        .find(|(index, _)| {
            *index == 0
                || !tag.as_bytes()[index.saturating_sub(1)].is_ascii_alphanumeric()
                    && tag.as_bytes()[index.saturating_sub(1)] != b'-'
                    && tag.as_bytes()[index.saturating_sub(1)] != b'_'
        })
        .map(|(index, _)| index + attribute.len())?;
    let value = tag[start..].trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    let quote = value.chars().next()?;
    let value = if quote == '\'' || quote == '"' {
        let end = value[quote.len_utf8()..].find(quote)? + quote.len_utf8();
        &value[quote.len_utf8()..end]
    } else {
        value.split_whitespace().next()?.trim_end_matches('>')
    };
    Some(value.to_string())
}

/// 把图片 data URL 解码为二进制候选。
fn data_url_candidate(text: &str) -> Result<Option<Candidate>> {
    let Some(rest) = text.strip_prefix("data:") else {
        return Ok(None);
    };
    let Some((meta, encoded)) = rest.split_once(',') else {
        bail!("invalid image data URL");
    };
    if !meta.ends_with(";base64") {
        bail!("image data URL must use base64 encoding");
    }
    let mime = meta
        .strip_suffix(";base64")
        .filter(|value| value.starts_with("image/"));
    Ok(Some(Candidate::Bytes {
        bytes: decode_base64(encoded)?,
        mime_hint: mime.map(str::to_string),
        source: "data_url",
    }))
}

/// 识别可匿名下载的 HTTP(S) 图片地址。
fn url_candidate(text: &str) -> Option<Candidate> {
    let url = text.trim();
    (url.starts_with("https://") || url.starts_with("http://"))
        .then(|| Candidate::Url(url.to_string()))
}

/// 解码带空白换行的标准或 URL-safe Base64。
fn decode_base64(text: &str) -> Result<Vec<u8>> {
    let compact: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(&compact)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(&compact))
        .context("invalid image base64")
}

/// 匿名下载 URL 图片，并限制响应大小和协议。
async fn download_image(client: &reqwest::Client, url: &str) -> Result<ImagePayload> {
    let parsed = reqwest::Url::parse(url).context("invalid image URL")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("image URL must use HTTP(S)");
    }
    let response = client.get(parsed).send().await?.error_for_status()?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_IMAGE_BYTES as u64)
    {
        bail!("downloaded image exceeds {} bytes", MAX_IMAGE_BYTES);
    }
    let mime_hint = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| value.starts_with("image/"))
        .map(str::to_string);
    let bytes = response.bytes().await?.to_vec();
    if bytes.len() > MAX_IMAGE_BYTES {
        bail!("downloaded image exceeds {} bytes", MAX_IMAGE_BYTES);
    }
    Ok(ImagePayload {
        bytes,
        mime_hint,
        source: "url",
    })
}

/// 创建图片下载客户端，限定重定向次数和请求时长。
///
/// 返回:
/// - 供生成响应下载图片的 HTTP 客户端
pub(super) fn client() -> Result<Arc<reqwest::Client>> {
    Ok(Arc::new(
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::limited(3))
            .timeout(Duration::from_secs(180))
            .build()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn decodes_json_base64_and_data_url() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"fake-png");
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [{"b64_json": encoded}, {"url": "data:image/png;base64,ZmFrZS1wbmc="}]
        }))
        .unwrap();
        let images = decode_response(body, Some("application/json"), client().unwrap())
            .await
            .unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].bytes, b"fake-png");
    }

    #[tokio::test]
    async fn extracts_image_from_html_block() {
        let body = br#"<html><img src = "data:image/png;base64,ZmFrZS1wbmc="></html>"#.to_vec();
        let images = decode_response(body, Some("text/html"), client().unwrap())
            .await
            .unwrap();
        assert_eq!(images[0].source, "data_url");
        assert_eq!(images[0].bytes, b"fake-png");
    }

    #[tokio::test]
    async fn downloads_url_without_forwarding_generation_headers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 1024];
            let length = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..length]).to_ascii_lowercase();
            assert!(!request.contains("authorization:"));
            let body = b"fake-png";
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
        });
        let body = serde_json::to_vec(&serde_json::json!({
            "data": [{"url": format!("http://{address}/image.png")}]
        }))
        .unwrap();
        let images = decode_response(body, Some("application/json"), client().unwrap())
            .await
            .unwrap();
        task.await.unwrap();
        assert_eq!(images[0].source, "url");
        assert_eq!(images[0].bytes, b"fake-png");
    }
}

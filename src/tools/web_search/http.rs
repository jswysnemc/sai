use anyhow::{bail, Result};
use reqwest::{Client, RequestBuilder};
use std::time::Duration;

pub(super) const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

/// 【网页搜索】【网络客户端】接收请求超时秒数；返回仅跟随同源重定向的客户端。
pub(super) fn client(timeout_seconds: u64) -> Result<Client> {
    Ok(Client::builder()
        .timeout(Duration::from_secs(timeout_seconds.clamp(1, 120)))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            // 1. 【网页搜索】【凭据边界】自定义 API Key 请求头和查询正文不能发送给其他来源
            if attempt.previous().len() >= 5
                || attempt
                    .previous()
                    .first()
                    .is_some_and(|url| url.origin() != attempt.url().origin())
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}

/// 【网页搜索】【响应读取】接收请求构造器；返回有界解码文本，错误不带查询地址或正文。
pub(super) async fn execute(request: RequestBuilder) -> Result<String> {
    let mut response = request.send().await.map_err(|error| error.without_url())?;
    if !response.status().is_success() {
        bail!(
            "search provider returned HTTP {}",
            response.status().as_u16()
        );
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        bail!("search response exceeds byte limit");
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut bytes = Vec::new();
    // 1. 【网页搜索】【流式限制】无 Content-Length 或分块响应也必须遵守实际字节上限
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| error.without_url())?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            bail!("search response exceeds byte limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    // 2. 【网页搜索】【字符解码】优先 BOM，其次响应声明编码，缺省使用 UTF-8
    let declared = content_type.split(';').skip(1).find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        key.eq_ignore_ascii_case("charset")
            .then(|| {
                encoding_rs::Encoding::for_label(value.trim().trim_matches(['\'', '"']).as_bytes())
            })
            .flatten()
    });
    let (encoding, offset) = encoding_rs::Encoding::for_bom(&bytes)
        .unwrap_or((declared.unwrap_or(encoding_rs::UTF_8), 0));
    let (text, _, _) = encoding.decode(&bytes[offset..]);
    if text.len() > MAX_RESPONSE_BYTES {
        bail!("decoded search response exceeds byte limit");
    }
    Ok(text.into_owned())
}

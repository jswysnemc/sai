use super::request::{FetchInput, Format};
use anyhow::{bail, Result};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, CONTENT_TYPE};
use std::{sync::OnceLock, time::Duration};

pub(super) const MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024;
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

pub(super) struct Response {
    pub bytes: Vec<u8>,
    pub content_type: String,
}

/// 【网页读取】【连接复用】无参数，返回共享 HTTP 客户端，重定向最多跟随十次。
fn client() -> Result<reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 10 {
                attempt.error("web fetch redirect limit exceeded")
            } else if !matches!(attempt.url().scheme(), "http" | "https") {
                attempt.error("web fetch redirect requires HTTP or HTTPS")
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    let _ = CLIENT.set(client.clone());
    Ok(CLIENT.get().cloned().unwrap_or(client))
}

/// 【网页读取】【有界请求】参数为已校验请求，返回 MIME 与原始字节；错误先处理状态且不回显查询地址。
pub(super) async fn fetch(input: &FetchInput) -> Result<Response> {
    if input.timeout == 0 {
        bail!("web fetch request timed out");
    }
    let accept = match input.format {
        Format::Text => "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
        Format::Html => "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, */*;q=0.1",
        Format::Markdown => "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
    };
    let mut response = client()?
        .get(input.url.clone())
        .header(ACCEPT, accept)
        .header(ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .timeout(Duration::from_secs(input.timeout))
        .send()
        .await
        .map_err(|error| error.without_url())?;
    // 1. 【网页读取】【状态优先】错误页不继续下载，即使它声明了超限正文
    if response.status().is_client_error() || response.status().is_server_error() {
        let category = if response.status().is_client_error() {
            "client"
        } else {
            "server"
        };
        bail!(
            "HTTP status {category} error ({})",
            response.status().as_u16()
        );
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        bail!("response too large (exceeds 5MB limit)");
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let mut bytes = Vec::new();
    // 2. 【网页读取】【流式上限】没有长度头或使用分块传输时，仍按实际读取字节限制内存
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| error.without_url())?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            bail!("response too large (exceeds 5MB limit)");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Response {
        bytes,
        content_type,
    })
}

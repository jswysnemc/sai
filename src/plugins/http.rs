use anyhow::{bail, Context, Result};
use sai_plugin_runtime::host::{HttpRequest, HttpResponse};
use sai_plugin_runtime::Capabilities;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// 【插件】【HTTP 执行】逐次校验请求及重定向，整个请求共用同一截止时间。
/// @param request 已校验请求；capabilities 为有效网络授权；allow_writes 为宿主调用权限
/// @returns 解码后仍满足字节上限的响应
pub(super) async fn execute(
    request: HttpRequest,
    capabilities: Capabilities,
    allow_writes: bool,
) -> Result<HttpResponse> {
    let limit = request.max_bytes;
    read_response(send(request, capabilities, allow_writes).await?, limit).await
}

/// 【插件】【请求发送】文本和二进制归档共用来源、重定向、凭据与截止时间校验。
/// @param request 请求；capabilities 为授权；allow_writes 为调用权限
/// @returns 尚未读取正文的最终响应
pub(super) async fn send(
    mut request: HttpRequest,
    capabilities: Capabilities,
    allow_writes: bool,
) -> Result<reqwest::Response> {
    let timeout = Duration::from_millis(request.timeout_ms.clamp(1, 120_000));
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    for redirects in 0..=5 {
        let url = capabilities.authorize_request(&request.method, &request.url, allow_writes)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .context("plugin HTTP request timed out")?;
        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .context("invalid plugin HTTP method")?;
        let mut builder = client.request(method, url.clone()).timeout(remaining);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = &request.body {
            builder = builder.body(body.clone());
        }
        let response = builder
            .send()
            .await
            .map_err(|error| error.without_url())
            .context("plugin HTTP request failed")?;
        let status = response.status().as_u16();
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            return Ok(response);
        }
        let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
            return Ok(response);
        };
        if redirects == 5 {
            bail!("plugin redirect limit exceeded");
        }
        let target = url
            .join(
                location
                    .to_str()
                    .context("invalid plugin redirect header")?,
            )
            .context("invalid plugin redirect target")?;
        // 1. 【插件】【重定向方法】按 HTTP 语义切换方法；保留 POST 的重定向仍需查询端点授权
        if ((status == 301 || status == 302) && request.method == "POST")
            || (status == 303 && request.method != "HEAD")
        {
            request.method = "GET".into();
            request.body = None;
            request.headers.retain(|name, _| {
                !matches!(
                    name.to_ascii_lowercase().as_str(),
                    "content-type" | "content-encoding"
                )
            });
        }
        // 2. 【插件】【跨来源凭据】不跨来源转发正文，额外认证头只发送到初始来源
        if url.origin() != target.origin() {
            if request.body.is_some() {
                bail!("plugin redirect cannot forward a request body across origins");
            }
            request.headers.retain(|name, _| {
                matches!(
                    name.to_ascii_lowercase().as_str(),
                    "accept" | "accept-language" | "user-agent"
                )
            });
        }
        request.url = target.to_string();
    }
    unreachable!("all redirect attempts return a response or an error")
}

/// 【插件】【响应读取】限制传输字节与解码后的 UTF-8 字节，保留 HTTP 状态供 Lua 处理。
/// @param response 网络响应；max_bytes 为本次请求的字节上限
/// @returns 状态、响应头与解码正文
async fn read_response(response: reqwest::Response, max_bytes: usize) -> Result<HttpResponse> {
    let status = response.status().as_u16();
    let headers: BTreeMap<String, String> = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect();
    let bytes = read_bytes(response, max_bytes).await?;
    let text = decode_body(&bytes, headers.get("content-type").map(String::as_str));
    if text.len() > max_bytes {
        bail!("decoded plugin HTTP response exceeds byte limit");
    }
    Ok(HttpResponse {
        status,
        headers,
        text,
    })
}

/// 【插件】【有界字节】限制实际传输正文，错误不会附带原始 URL。
/// @param response 最终响应；max_bytes 为字节上限
/// @returns 有界原始正文
pub(super) async fn read_bytes(
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|size| size > max_bytes as u64)
    {
        bail!("plugin HTTP response exceeds byte limit");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| error.without_url())
        .context("read plugin HTTP response")?
    {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            bail!("plugin HTTP response exceeds byte limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// 【插件】【文本解码】尊重 HTTP charset，并在缺省情况下使用 UTF-8。
/// @param bytes 原始正文；content_type 为可选 Content-Type
/// @returns 解码文本
fn decode_body(bytes: &[u8], content_type: Option<&str>) -> String {
    let charset = content_type.and_then(|value| {
        value.split(';').find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            name.eq_ignore_ascii_case("charset")
                .then(|| value.trim().trim_matches(['\'', '"']))
        })
    });
    let encoding = charset
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let (text, _, _) = encoding.decode(bytes);
    text.into_owned()
}

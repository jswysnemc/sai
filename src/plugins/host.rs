use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{HttpRequest, HttpResponse, PluginHost};
use std::collections::BTreeMap;
use std::time::Duration;

/// 【插件】【Sai 宿主】执行跨平台 HTTP 请求，重定向和响应大小继续受插件授权约束。
pub(super) struct SaiPluginHost;

#[async_trait]
impl PluginHost for SaiPluginHost {
    /// 【插件】【HTTP 执行】使用已有 Rust 网络栈执行请求并限制重定向。
    /// @param request 已校验请求；allowed_origins 为可访问来源
    /// @returns UTF-8 解码后的有界响应，不把 URL 凭据写入错误
    async fn http(
        &self,
        request: HttpRequest,
        allowed_origins: Vec<String>,
    ) -> Result<HttpResponse> {
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            let origin = attempt.url().origin().ascii_serialization();
            if attempt.previous().len() >= 5 {
                return attempt.error("plugin redirect limit exceeded");
            }
            if !allowed_origins.contains(&origin) {
                return attempt.error("plugin redirect origin is not allowed");
            }
            if !attempt.url().username().is_empty() || attempt.url().password().is_some() {
                return attempt.error("plugin redirect contains credentials");
            }
            attempt.follow()
        });
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(request.timeout_ms.clamp(1, 30_000)))
            .redirect(policy)
            .build()?;
        let method = reqwest::Method::from_bytes(request.method.as_bytes())
            .context("invalid plugin HTTP method")?;
        let mut builder = client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let mut response = builder
            .send()
            .await
            .map_err(|error| error.without_url())
            .context("plugin HTTP request failed")?;
        if response
            .content_length()
            .is_some_and(|size| size > request.max_bytes as u64)
        {
            bail!("plugin HTTP response exceeds byte limit");
        }
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
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| error.without_url())
            .context("read plugin HTTP response")?
        {
            if bytes.len().saturating_add(chunk.len()) > request.max_bytes {
                bail!("plugin HTTP response exceeds byte limit");
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = decode_body(&bytes, headers.get("content-type").map(String::as_str));
        if text.len() > request.max_bytes {
            bail!("decoded plugin HTTP response exceeds byte limit");
        }
        Ok(HttpResponse {
            status,
            headers,
            text,
        })
    }
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
